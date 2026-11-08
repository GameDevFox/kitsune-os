//! Builds the kernel and runs it on the real Pi over JTAG, through a running OpenOCD.

use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};

/// OpenOCD's command port. Commands and replies each end with this byte.
const OPENOCD_ADDRESS: &str = "127.0.0.1:6666";
const OPENOCD_END: u8 = 0x1a;
const OPENOCD_TIMEOUT: Duration = Duration::from_secs(30);
const READ_BUFFER_SIZE: usize = 4096;

const TARGET: &str = "bcm2711.cpu0";
const HALT_TIMEOUT_MS: u32 = 2000;
/// Must match `board/raspi4/link.ld`.
const LOAD_ADDRESS: u64 = 0x80000;
/// Must match `UPLOAD_BASE` in `board/raspi4/mod.rs`. The kernel reads it after boot, whose
/// cache cleaning means it can't be looking at anything older than what's loaded here.
const UPLOAD_ADDRESS: u64 = 0x1100_0000;
/// Long enough for a new kernel to reach its main loop, and start reading serial.
const KERNEL_START_TIME: Duration = Duration::from_millis(1500);
/// JTAG can't reach the core for several seconds after a reboot, so keep trying for this long.
const REBOOT_WAIT: Duration = Duration::from_secs(25);
const RETRY_PAUSE: Duration = Duration::from_secs(1);

/// The CPSR bit that is set while the core is in 32-bit mode.
const CPSR_AARCH32: u64 = 1 << 4;
/// The debug, SError, IRQ and FIQ mask bits.
const CPSR_MASK_ALL: u64 = 0b1111 << 6;

const USAGE: &str = "\
Builds the kernel and runs it on the real Pi over JTAG. Its output appears in `cargo uart`.
Needs OpenOCD running: openocd -f tools/jtag/pi4.cfg

Usage: cargo pi [--install]

--install also replaces the kernel on the card, once the new one is running, which goes
far faster than `cargo upload kernel` does, as JTAG carries it.";

/// Finds the hex value in a line like `cpsr (/32): 0x600003c5`.
fn parse_register(reply: &str) -> Result<u64, String> {
    let (_, value) = reply.rsplit_once("0x").ok_or(format!("no value in: {reply:?}"))?;
    let digits = value.split_whitespace().next().unwrap_or("");
    u64::from_str_radix(digits, 16).map_err(|_| format!("bad value in: {reply:?}"))
}

/// Loads an image to an address, then verifies it. OpenOCD's own `verify_image` can't be
/// used, as it needs a checksum routine this target doesn't have, so the image is read
/// back into the `read_back` file and compared here.
fn load_and_verify(
    run: &mut impl FnMut(&str) -> Result<String, String>,
    path: &Path,
    image: &[u8],
    read_back: &Path,
    address: u64,
) -> Result<(), String> {
    let loaded = run(&format!("load_image {{{}}} {address:#x} bin", path.display()))?;
    if !loaded.contains("downloaded") {
        return Err(format!("load failed, and the core is left halted: {:?}", loaded.trim()));
    }
    // A leftover file must not be able to pass for a successful read.
    match std::fs::remove_file(read_back) {
        Err(error) if error.kind() != ErrorKind::NotFound => {
            return Err(format!("could not remove {}: {error}", read_back.display()));
        }
        _ => {}
    }
    run(&format!("dump_image {{{}}} {address:#x} {}", read_back.display(), image.len()))?;
    let actual = std::fs::read(read_back)
        .map_err(|error| format!("verify failed, and the core is left halted: nothing read back: {error}"))?;
    if actual != image {
        return Err(format!(
            "verify failed: the {} bytes read back don't match the {} sent, and the core is left halted",
            actual.len(),
            image.len()
        ));
    }
    Ok(())
}

/// Halts core 0, loads the image, verifies it, and starts it, also leaving a copy in the
/// upload memory if asked. `run` sends one OpenOCD command and returns its reply.
fn deploy(
    run: &mut impl FnMut(&str) -> Result<String, String>,
    path: &Path,
    image: &[u8],
    read_back: &Path,
    copy_for_install: bool,
) -> Result<(), String> {
    run(&format!("targets {TARGET}"))?;
    run(&format!("halt {HALT_TIMEOUT_MS}"))?;
    let state = run(&format!("{TARGET} curstate"))?;
    if state.trim() != "halted" {
        return Err(format!("core 0 did not halt (state: {}). Try restarting OpenOCD, then power-cycling the Pi.", state.trim()));
    }

    let cpsr = parse_register(&run("reg cpsr")?)?;
    if cpsr & CPSR_AARCH32 != 0 {
        run("resume")?;
        return Err("the core is in 32-bit mode, so the card booted the old kernel. \
                    Run scripts/card.py new with the card in the reader, then boot from it."
            .to_string());
    }

    load_and_verify(run, path, image, read_back, LOAD_ADDRESS)?;
    if copy_for_install {
        load_and_verify(run, path, image, read_back, UPLOAD_ADDRESS)?;
    }

    // An old kernel may have let interrupts in, and one arriving before the new kernel's
    // first instruction masks them would jump into the replaced code.
    run(&format!("reg cpsr {:#x}", cpsr | CPSR_MASK_ALL))?;
    // Only a real boot has the firmware's device tree pointer in x0. Clearing it tells the
    // kernel to keep the one it saved then, rather than whatever the old kernel left here.
    run("reg x0 0")?;
    run(&format!("resume {LOAD_ADDRESS:#x}"))?;
    let state = run(&format!("{TARGET} curstate"))?;
    if state.trim() != "running" {
        return Err(format!("core 0 did not start (state: {})", state.trim()));
    }
    Ok(())
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn build_image(root: &Path) -> Result<PathBuf, String> {
    let output = Command::new("scripts/build-image")
        .current_dir(root)
        .output()
        .map_err(|error| format!("could not run scripts/build-image: {error}"))?;
    if !output.status.success() {
        return Err(format!("build failed:\n{}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(root.join("target/kernel-only.img"))
}

fn connect() -> Result<TcpStream, String> {
    let stream = TcpStream::connect(OPENOCD_ADDRESS).map_err(|error| {
        format!("could not reach OpenOCD at {OPENOCD_ADDRESS} (start it with: openocd -f tools/jtag/pi4.cfg): {error}")
    })?;
    stream.set_read_timeout(Some(OPENOCD_TIMEOUT)).map_err(|error| error.to_string())?;
    Ok(stream)
}

fn send_command(stream: &mut TcpStream, command: &str) -> Result<String, String> {
    let lost = |error: std::io::Error| format!("lost OpenOCD during `{command}`: {error}");
    stream.write_all(command.as_bytes()).map_err(lost)?;
    stream.write_all(&[OPENOCD_END]).map_err(lost)?;

    let mut reply = Vec::new();
    let mut buffer = [0; READ_BUFFER_SIZE];
    while !reply.contains(&OPENOCD_END) {
        let count = stream.read(&mut buffer).map_err(lost)?;
        if count == 0 {
            return Err(format!("OpenOCD closed the connection during `{command}`"));
        }
        reply.extend_from_slice(&buffer[..count]);
    }
    reply.truncate(reply.iter().position(|&byte| byte == OPENOCD_END).unwrap_or(reply.len()));
    Ok(String::from_utf8_lossy(&reply).into_owned())
}

/// Whether a failure could just be that the Pi has only just rebooted.
fn is_temporary(message: &str) -> bool {
    message.contains("did not halt") || message.contains("load failed")
}

/// Has the new kernel write the copy of itself in its upload memory to the card.
fn install(root: &Path, image: &Path) -> Result<(), String> {
    std::thread::sleep(KERNEL_START_TIME);
    let status = Command::new("cargo")
        .args(["upload", "kernel"])
        .arg(image)
        .args(["--from-memory", "--really-replace-the-boot-kernel"])
        .current_dir(root)
        .status()
        .map_err(|error| format!("could not run cargo upload: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("the kernel is running, but it isn't on the card".to_string())
    }
}

fn run(install_too: bool) -> Result<(), String> {
    let root = workspace_root();
    let image = build_image(&root)?;
    // Connect after building, so that a compile error is the first thing reported.
    let mut stream = connect()?;

    let bytes = std::fs::read(&image).map_err(|error| format!("could not read {}: {error}", image.display()))?;
    let start = Instant::now();
    let read_back = root.join("target/kitsune-next.readback");
    loop {
        match deploy(&mut |command| send_command(&mut stream, command), &image, &bytes, &read_back, install_too) {
            Ok(()) => break,
            Err(message) if is_temporary(&message) && start.elapsed() < REBOOT_WAIT => {
                eprintln!("pi: can't reach the core yet, which is normal just after a reboot. Retrying.");
                std::thread::sleep(RETRY_PAUSE);
            }
            Err(message) => return Err(message),
        }
    }
    println!("pi: running, {} bytes loaded and verified in {:.2}s", bytes.len(), start.elapsed().as_secs_f32());
    if install_too {
        install(&root, &image)?;
    }
    Ok(())
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let install_too = arguments.iter().any(|argument| argument == "--install") && arguments.len() == 1;
    if !arguments.is_empty() && !install_too {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    match run(install_too) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("pi: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AARCH64_EL1: &str = "cpsr (/32): 0x600003c5";
    const AARCH32_SVC: &str = "cpsr (/32): 0x800001d3";
    const IMAGE: &[u8] = &[0xa1, 0x00, 0x38, 0xff];

    /// A read-back path that no other test shares, as tests run in parallel.
    fn read_back_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("pi-test-{}-{name}", std::process::id()))
    }

    /// Pretends to be OpenOCD, recording the commands it receives.
    struct FakeOpenOcd {
        commands: Vec<String>,
        cpsr: &'static str,
        state_when_halted: &'static str,
        load_reply: &'static str,
        /// What `dump_image` writes, or `None` if it writes nothing.
        memory: Option<&'static [u8]>,
        read_back: PathBuf,
        resumed: bool,
        copy_for_install: bool,
    }

    impl FakeOpenOcd {
        fn new(test_name: &str) -> Self {
            Self {
                commands: Vec::new(),
                cpsr: AARCH64_EL1,
                state_when_halted: "halted",
                load_reply: "4 bytes written at address 0x00080000\ndownloaded 4 bytes in 0.04s (1 KiB/s)\n",
                memory: Some(IMAGE),
                read_back: read_back_path(test_name),
                resumed: false,
                copy_for_install: false,
            }
        }

        fn deploy(&mut self) -> Result<(), String> {
            let mut pending = std::mem::take(&mut self.commands);
            let read_back = self.read_back.clone();
            let copy_for_install = self.copy_for_install;
            let result = deploy(
                &mut |command| {
                    pending.push(command.to_string());
                    Ok(self.reply(command))
                },
                Path::new("/k.img"),
                IMAGE,
                &read_back,
                copy_for_install,
            );
            self.commands = pending;
            result
        }

        fn reply(&mut self, command: &str) -> String {
            if command.starts_with("resume") {
                self.resumed = true;
            }
            if command.starts_with("dump_image") {
                if let Some(memory) = self.memory {
                    std::fs::write(&self.read_back, memory).unwrap();
                }
            }
            let reply = match command.split_whitespace().next().unwrap_or("") {
                "reg" => self.cpsr,
                "load_image" => self.load_reply,
                "bcm2711.cpu0" if self.resumed => "running",
                "bcm2711.cpu0" => self.state_when_halted,
                _ => "",
            };
            reply.to_string()
        }

        fn ran(&self, prefix: &str) -> bool {
            self.commands.iter().any(|command| command.starts_with(prefix))
        }
    }

    #[test]
    fn a_normal_deploy_runs_every_step_in_order() {
        let mut openocd = FakeOpenOcd::new("a_normal_deploy_runs_every_step_in_order");
        assert_eq!(openocd.deploy(), Ok(()));
        assert_eq!(
            openocd.commands,
            [
                "targets bcm2711.cpu0",
                "halt 2000",
                "bcm2711.cpu0 curstate",
                "reg cpsr",
                "load_image {/k.img} 0x80000 bin",
                format!("dump_image {{{}}} 0x80000 4", openocd.read_back.display()).as_str(),
                "reg cpsr 0x600003c5",
                "reg x0 0",
                "resume 0x80000",
                "bcm2711.cpu0 curstate",
            ]
        );
    }

    #[test]
    fn the_new_kernel_starts_with_interrupts_masked() {
        // An old kernel with IRQs let in: bit 7 clear.
        const IRQS_ON: &str = "cpsr (/32): 0x60000345";
        let mut openocd = FakeOpenOcd::new("the_new_kernel_starts_with_interrupts_masked");
        openocd.cpsr = IRQS_ON;
        assert_eq!(openocd.deploy(), Ok(()));
        let position = |wanted: &str| openocd.commands.iter().position(|command| command == wanted).unwrap();
        assert!(position("reg cpsr 0x600003c5") < position("resume 0x80000"));
    }

    #[test]
    fn an_install_also_loads_and_verifies_a_copy_before_starting() {
        let mut openocd = FakeOpenOcd::new("an_install_also_loads_and_verifies_a_copy_before_starting");
        openocd.copy_for_install = true;
        assert_eq!(openocd.deploy(), Ok(()));
        let position = |wanted: &str| openocd.commands.iter().position(|command| command.starts_with(wanted)).unwrap();
        let copy = position("load_image {/k.img} 0x11000000 bin");
        assert!(copy > position("load_image {/k.img} 0x80000 bin"));
        assert!(position(&format!("dump_image {{{}}} 0x11000000", openocd.read_back.display())) > copy);
        assert!(position("resume 0x80000") > copy);
    }

    #[test]
    fn only_reaching_the_core_is_worth_retrying() {
        let mut openocd = FakeOpenOcd::new("only_reaching_the_core_is_worth_retrying");
        openocd.state_when_halted = "unknown";
        assert!(is_temporary(&openocd.deploy().unwrap_err()));

        let mut openocd = FakeOpenOcd::new("only_reaching_the_core_is_worth_retrying_2");
        openocd.memory = Some(b"DIFF");
        assert!(!is_temporary(&openocd.deploy().unwrap_err()));
    }

    #[test]
    fn a_normal_run_leaves_the_upload_memory_alone() {
        let mut openocd = FakeOpenOcd::new("a_normal_run_leaves_the_upload_memory_alone");
        assert_eq!(openocd.deploy(), Ok(()));
        assert!(!openocd.commands.iter().any(|command| command.contains("0x11000000")));
    }

    #[test]
    fn a_core_in_32_bit_mode_is_resumed_and_nothing_is_loaded() {
        let mut openocd = FakeOpenOcd::new("a_core_in_32_bit_mode_is_resumed_and_nothing_is_loaded");
        openocd.cpsr = AARCH32_SVC;

        let error = openocd.deploy().unwrap_err();

        assert!(error.contains("32-bit"), "{error}");
        assert!(!openocd.ran("load_image"));
        assert_eq!(openocd.commands.last().map(String::as_str), Some("resume"));
    }

    #[test]
    fn a_core_that_will_not_halt_stops_before_loading() {
        let mut openocd = FakeOpenOcd::new("a_core_that_will_not_halt_stops_before_loading");
        openocd.state_when_halted = "unknown";
        assert!(openocd.deploy().unwrap_err().contains("did not halt"));
        assert!(!openocd.ran("load_image"));
    }

    #[test]
    fn a_failed_load_never_starts_the_kernel() {
        let mut openocd = FakeOpenOcd::new("a_failed_load_never_starts_the_kernel");
        openocd.load_reply = "";
        assert!(openocd.deploy().unwrap_err().contains("load failed"));
        assert!(!openocd.ran("dump_image"));
        assert!(!openocd.ran("resume"));
    }

    #[test]
    fn different_bytes_read_back_never_start_the_kernel() {
        let mut openocd = FakeOpenOcd::new("different_bytes_read_back_never_start_the_kernel");
        openocd.memory = Some(&[0xa1, 0x00, 0x39, 0xff]);
        assert!(openocd.deploy().unwrap_err().contains("verify failed"));
        assert!(!openocd.ran("resume"));
    }

    #[test]
    fn fewer_bytes_read_back_never_start_the_kernel() {
        let mut openocd = FakeOpenOcd::new("fewer_bytes_read_back_never_start_the_kernel");
        openocd.memory = Some(&[0xa1, 0x00, 0x38]);
        assert!(openocd.deploy().unwrap_err().contains("verify failed"));
        assert!(!openocd.ran("resume"));
    }

    #[test]
    fn a_leftover_file_cannot_pass_for_a_successful_read() {
        let mut openocd = FakeOpenOcd::new("leftover");
        std::fs::write(&openocd.read_back, IMAGE).unwrap();
        openocd.memory = None;
        assert!(openocd.deploy().unwrap_err().contains("verify failed"));
        assert!(!openocd.ran("resume"));
    }

    #[test]
    fn a_register_value_is_parsed() {
        assert_eq!(parse_register(AARCH64_EL1), Ok(0x6000_03c5));
        assert_eq!(parse_register("(33) cpsr (/32): 0x800001d3 (dirty)"), Ok(0x8000_01d3));
    }

    #[test]
    fn a_reply_without_a_value_is_an_error() {
        assert!(parse_register("").is_err());
        assert!(parse_register("cpsr: 0xZZ").is_err());
    }
}
