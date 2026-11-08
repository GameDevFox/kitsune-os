//! Checks the FAT code against real filesystems, with Linux as the independent judge:
//! `mkfs.vfat` formats them, Linux's own driver creates their files, this crate overwrites
//! one, then `fsck.vfat` and Linux's driver read the result. Needs `mkfs.vfat`, `fsck.vfat`
//! and `udisksctl`, and only touches images and loop devices it made itself.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use fat::{Block, Blocks, DeviceError, FatError, File, Volume, BLOCK_BYTES};

const IMAGE_MEGABYTES: u64 = 64;
const FAT32_SECTORS_PER_CLUSTER: [u32; 2] = [1, 8];
const SECOND_NAME: &str = "kitsune-next.img";
const TARGET_CLUSTERS: usize = 5;
/// Enough for the root directory to outgrow a one-block cluster, several times over.
const DIRECTORY_FILES: usize = 40;
const ROOT_CLUSTER: u32 = 2;
const ENTRY_BYTES: usize = 32;
const LONG_NAME_ATTRIBUTE: u8 = 0x0F;
const ENTRY_ATTRIBUTES_AT: usize = 11;
const LONG_CHECKSUM_AT: usize = 13;

/// udisks mounts by label, so tests take turns.
static MOUNTS: Mutex<()> = Mutex::new(());

fn run(program: &str, arguments: &[&str]) -> String {
    let output = Command::new(program).args(arguments).output().unwrap_or_else(|error| panic!("{program}: {error}"));
    assert!(output.status.success(), "{program} {arguments:?}: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Mounts the image with Linux's driver for the length of `action`.
fn with_mount<T>(image: &Path, action: impl FnOnce(&Path) -> T) -> T {
    let _turn = MOUNTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let text = run("udisksctl", &["loop-setup", "--no-user-interaction", "-f", image.to_str().unwrap()]);
    let device = text.split_whitespace().find(|word| word.starts_with("/dev/loop")).unwrap().trim_end_matches('.').to_string();
    run("udisksctl", &["mount", "--no-user-interaction", "-b", &device]);
    let mount_point = run("lsblk", &["-no", "MOUNTPOINT", &device]).trim().to_string();
    let result = action(Path::new(&mount_point));
    run("sync", &[]);
    run("udisksctl", &["unmount", "--no-user-interaction", "-b", &device]);
    run("udisksctl", &["loop-delete", "--no-user-interaction", "-b", &device]);
    result
}

fn pattern(length: usize, seed: u8) -> Vec<u8> {
    (0..length).map(|index| (index % 251) as u8 ^ seed).collect()
}

struct FileDevice {
    bytes: Vec<u8>,
    writes: Vec<u32>,
}

impl Blocks for FileDevice {
    fn read(&mut self, block: u32, data: &mut Block) -> Result<(), DeviceError> {
        let start = block as usize * BLOCK_BYTES;
        data.copy_from_slice(self.bytes.get(start..start + BLOCK_BYTES).ok_or(DeviceError)?);
        Ok(())
    }

    fn write(&mut self, block: u32, data: &Block) -> Result<(), DeviceError> {
        let start = block as usize * BLOCK_BYTES;
        self.bytes.get_mut(start..start + BLOCK_BYTES).ok_or(DeviceError)?.copy_from_slice(data);
        self.writes.push(block);
        Ok(())
    }
}

/// A formatted image holding a fragmented file, with a long name, among others.
struct Fixture {
    path: PathBuf,
    target_size: usize,
    other_a: Vec<u8>,
    other_c: Vec<u8>,
}

fn make_fixture(test: &str, sectors_per_cluster: u32) -> Fixture {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{test}-{sectors_per_cluster}"));
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("image");
    let _ = fs::remove_file(&path);
    fs::File::create(&path).unwrap().set_len(IMAGE_MEGABYTES * 1024 * 1024).unwrap();
    run("mkfs.vfat", &["-F", "32", "-s", &sectors_per_cluster.to_string(), "-n", "FATTEST", path.to_str().unwrap()]);

    let cluster_bytes = sectors_per_cluster as usize * BLOCK_BYTES;
    let chunk = 3 * cluster_bytes;
    let target_size = TARGET_CLUSTERS * cluster_bytes + 123;
    let other_a = pattern(chunk, 0x11);
    let other_c = pattern(chunk, 0x33);
    with_mount(&path, |mount| {
        fs::write(mount.join("a.bin"), &other_a).unwrap();
        fs::write(mount.join("b.bin"), pattern(cluster_bytes, 0x22)).unwrap();
        fs::write(mount.join("c.bin"), &other_c).unwrap();
        run("sync", &[]);
        fs::remove_file(mount.join("b.bin")).unwrap();
        // Growing another file in turn with the target leaves the target's clusters apart.
        let mut target = fs::File::create(mount.join(SECOND_NAME)).unwrap();
        let mut neighbor = fs::File::create(mount.join("d.bin")).unwrap();
        let contents = pattern(target_size, 0x66);
        for piece in contents.chunks(cluster_bytes) {
            target.write_all(piece).unwrap();
            target.sync_all().unwrap();
            neighbor.write_all(&pattern(cluster_bytes, 0x44)).unwrap();
            neighbor.sync_all().unwrap();
        }
        fs::write(mount.join("kitsune.img"), pattern(chunk, 0x77)).unwrap();
    });
    Fixture { path, target_size, other_a, other_c }
}

fn load(fixture: &Fixture) -> FileDevice {
    FileDevice { bytes: fs::read(&fixture.path).unwrap(), writes: Vec::new() }
}

fn assert_fsck_clean(path: &Path) {
    let output = Command::new("fsck.vfat").args(["-n", "-v", path.to_str().unwrap()]).output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "fsck: {text}");
    assert!(!text.contains("Dirty") && !text.contains("corrupt") && !text.contains("Free cluster summary wrong"), "fsck: {text}");
}

#[test]
fn an_overwrite_is_read_back_by_linux_and_changes_nothing_else() {
    for sectors_per_cluster in FAT32_SECTORS_PER_CLUSTER {
        let fixture = make_fixture("overwrite", sectors_per_cluster);
        assert_fsck_clean(&fixture.path);
        let mut device = load(&fixture);
        let before = device.bytes.clone();

        let volume = Volume::open(&mut device).unwrap();
        let file = volume.find(&mut device, SECOND_NAME).unwrap();
        assert_eq!(file.size as usize, fixture.target_size);
        let contents = pattern(fixture.target_size - 700, 0x99);
        volume.overwrite(&mut device, &file, &contents).unwrap();

        // Every changed block must be one the writer wrote, and those must be the file's.
        let block_count = fixture.target_size.div_ceil(BLOCK_BYTES);
        assert_eq!(device.writes.len(), block_count);
        let mut changed = Vec::new();
        for (index, (old, new)) in before.chunks_exact(BLOCK_BYTES).zip(device.bytes.chunks_exact(BLOCK_BYTES)).enumerate() {
            if old != new {
                changed.push(index as u32);
            }
        }
        assert!(!changed.is_empty());
        assert!(changed.iter().all(|block| device.writes.contains(block)), "a block changed outside the writes");
        // It really was fragmented, or this test isn't testing that.
        let contiguous = device.writes.windows(2).all(|pair| pair[1] == pair[0] + 1);
        assert!(!contiguous, "the fixture's file wasn't fragmented");

        fs::write(&fixture.path, &device.bytes).unwrap();
        assert_fsck_clean(&fixture.path);
        with_mount(&fixture.path, |mount| {
            let mut expected = contents.clone();
            expected.resize(fixture.target_size, 0);
            assert_eq!(fs::read(mount.join(SECOND_NAME)).unwrap(), expected);
            assert_eq!(fs::read(mount.join("a.bin")).unwrap(), fixture.other_a);
            assert_eq!(fs::read(mount.join("c.bin")).unwrap(), fixture.other_c);
            assert_eq!(fs::read(mount.join("kitsune.img")).unwrap(), pattern(fixture.other_a.len(), 0x77));
        });
    }
}

#[test]
fn names_are_found_long_or_short_and_ignoring_case() {
    let fixture = make_fixture("names", 1);
    let mut device = load(&fixture);
    let volume = Volume::open(&mut device).unwrap();
    let long = volume.find(&mut device, SECOND_NAME).unwrap();
    assert_eq!(long, volume.find(&mut device, "KITSUNE-NEXT.IMG").unwrap());
    assert_eq!(volume.find(&mut device, "kitsune.img").unwrap().size as usize, fixture.other_a.len());
    assert_ne!(long, volume.find(&mut device, "kitsune.img").unwrap());
    assert_eq!(volume.find(&mut device, "a.bin").unwrap().size as usize, fixture.other_a.len());
}

#[test]
fn a_long_name_with_the_wrong_checksum_does_not_match() {
    let fixture = make_fixture("checksum", 1);
    let mut device = load(&fixture);
    let volume = Volume::open(&mut device).unwrap();
    assert!(volume.find(&mut device, SECOND_NAME).is_ok());
    // Break the checksum of every long-name entry on the volume's first root block.
    let reserved = usize::from(u16::from_le_bytes([device.bytes[14], device.bytes[15]]));
    let fat_blocks = u32::from_le_bytes(device.bytes[36..40].try_into().unwrap()) as usize;
    let root_start = (reserved + usize::from(device.bytes[16]) * fat_blocks) * BLOCK_BYTES;
    let mut broken = 0;
    for at in (root_start..root_start + BLOCK_BYTES).step_by(ENTRY_BYTES) {
        if device.bytes[at + ENTRY_ATTRIBUTES_AT] == LONG_NAME_ATTRIBUTE {
            device.bytes[at + LONG_CHECKSUM_AT] ^= 0x55;
            broken += 1;
        }
    }
    assert!(broken > 0);
    assert_eq!(volume.find(&mut device, SECOND_NAME), Err(FatError::NotFound));
}

#[test]
fn a_deleted_file_with_a_long_name_is_not_found() {
    let fixture = make_fixture("deleted", 1);
    with_mount(&fixture.path, |mount| {
        fs::write(mount.join("a-file-with-a-long-name.bin"), [1, 2, 3]).unwrap();
        run("sync", &[]);
        fs::remove_file(mount.join("a-file-with-a-long-name.bin")).unwrap();
    });
    let mut device = load(&fixture);
    let volume = Volume::open(&mut device).unwrap();
    assert_eq!(volume.find(&mut device, "a-file-with-a-long-name.bin"), Err(FatError::NotFound));
    assert!(volume.find(&mut device, SECOND_NAME).is_ok());
}

#[test]
fn a_directory_spanning_clusters_is_followed_and_a_bad_chain_in_it_is_refused() {
    let fixture = make_fixture("directory", 1);
    with_mount(&fixture.path, |mount| {
        for number in 0..DIRECTORY_FILES {
            fs::write(mount.join(format!("f{number}.bin")), [1]).unwrap();
        }
    });
    let mut device = load(&fixture);
    let volume = Volume::open(&mut device).unwrap();
    let last = format!("f{}.bin", DIRECTORY_FILES - 1);
    assert!(volume.find(&mut device, &last).is_ok());

    // The root directory's own cluster is number 2 on a fresh filesystem.
    for value in [0, 1, 0x0FFF_FFF0, 0x0FFF_FFF7] {
        let mut broken = FileDevice { bytes: device.bytes.clone(), writes: Vec::new() };
        set_fat_entry(&mut broken, ROOT_CLUSTER, value);
        assert_eq!(volume.find(&mut broken, &last), Err(FatError::BadChain), "{value:#x}");
    }
}

#[test]
fn deleted_and_missing_files_are_not_found() {
    let fixture = make_fixture("missing", 1);
    let mut device = load(&fixture);
    let volume = Volume::open(&mut device).unwrap();
    for name in ["b.bin", "e.bin", "nothing.img", "kitsune-nex.img", "kitsune-next.im", "kitsune-next.img2"] {
        assert_eq!(volume.find(&mut device, name), Err(FatError::NotFound), "{name}");
    }
}

/// Points a FAT entry at something else, in both copies of the FAT.
fn set_fat_entry(device: &mut FileDevice, cluster: u32, value: u32) {
    let reserved = usize::from(u16::from_le_bytes([device.bytes[14], device.bytes[15]]));
    let fat_blocks = u32::from_le_bytes(device.bytes[36..40].try_into().unwrap()) as usize;
    for copy in 0..usize::from(device.bytes[16]) {
        let at = (reserved + copy * fat_blocks) * BLOCK_BYTES + cluster as usize * 4;
        device.bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
}

#[test]
fn a_bad_chain_is_refused_before_anything_is_written() {
    let fixture = make_fixture("chains", 1);
    let clean = load(&fixture);
    let reference = |device: &mut FileDevice| {
        let volume = Volume::open(device).unwrap();
        let file = volume.find(device, SECOND_NAME).unwrap();
        (volume, file)
    };
    let cases: [(&str, u32); 4] = [("free", 0), ("bad mark", 0x0FFF_FFF7), ("past the end", 0x0FFF_FFF0), ("reserved", 1)];
    for (label, value) in cases {
        let mut device = FileDevice { bytes: clean.bytes.clone(), writes: Vec::new() };
        let (volume, file) = reference(&mut device);
        set_fat_entry(&mut device, file.first_cluster, value);
        assert_eq!(volume.overwrite(&mut device, &file, &[1]), Err(FatError::BadChain), "{label}");
        assert!(device.writes.is_empty(), "{label} wrote something");
    }

    // A chain that ends too soon, or runs on past the file's size.
    let mut device = FileDevice { bytes: clean.bytes.clone(), writes: Vec::new() };
    let (volume, file) = reference(&mut device);
    set_fat_entry(&mut device, file.first_cluster, 0x0FFF_FFFF);
    assert_eq!(volume.overwrite(&mut device, &file, &[1]), Err(FatError::BadChain));
    assert!(device.writes.is_empty());

    let mut device = FileDevice { bytes: clean.bytes.clone(), writes: Vec::new() };
    let (volume, file) = reference(&mut device);
    let short = File { size: BLOCK_BYTES as u32, ..file };
    assert_eq!(volume.overwrite(&mut device, &short, &[1]), Err(FatError::BadChain));
    assert!(device.writes.is_empty());

    // A loop in the chain can't run forever.
    let mut device = FileDevice { bytes: clean.bytes.clone(), writes: Vec::new() };
    let (volume, file) = reference(&mut device);
    set_fat_entry(&mut device, file.first_cluster, file.first_cluster);
    assert_eq!(volume.overwrite(&mut device, &file, &[1]), Err(FatError::BadChain));
    assert!(device.writes.is_empty());
}

#[test]
fn data_too_big_for_the_file_is_refused() {
    let fixture = make_fixture("toobig", 1);
    let mut device = load(&fixture);
    let volume = Volume::open(&mut device).unwrap();
    let file = volume.find(&mut device, SECOND_NAME).unwrap();
    assert_eq!(volume.overwrite(&mut device, &file, &pattern(fixture.target_size + 1, 1)), Err(FatError::TooLarge));
    assert!(device.writes.is_empty());
}

#[test]
fn an_empty_file_or_zero_cluster_is_refused() {
    let fixture = make_fixture("empty", 1);
    let mut device = load(&fixture);
    let volume = Volume::open(&mut device).unwrap();
    let empty = File { first_cluster: 0, size: 0 };
    assert_eq!(volume.overwrite(&mut device, &empty, &[]), Err(FatError::BadChain));
    let no_cluster = File { first_cluster: 0, size: 100 };
    assert_eq!(volume.overwrite(&mut device, &no_cluster, &[1]), Err(FatError::BadChain));
    assert!(device.writes.is_empty());
}

#[test]
fn a_filesystem_that_is_not_fat32_is_refused() {
    let fixture = make_fixture("notfat", 1);
    let clean = load(&fixture).bytes;
    let cases: [(&str, usize, u8); 3] = [("no signature", 510, 0), ("sector size", 11, 1), ("fat16 root", 17, 1)];
    for (label, at, value) in cases {
        let mut bytes = clean.clone();
        bytes[at] = value;
        let mut device = FileDevice { bytes, writes: Vec::new() };
        assert!(matches!(Volume::open(&mut device), Err(FatError::NotFat32)), "{label}");
    }
    let mut device = FileDevice { bytes: vec![0; 4096], writes: Vec::new() };
    assert!(matches!(Volume::open(&mut device), Err(FatError::NotFat32)));
}

struct FailingWrites(FileDevice);

impl Blocks for FailingWrites {
    fn read(&mut self, block: u32, data: &mut Block) -> Result<(), DeviceError> {
        self.0.read(block, data)
    }

    fn write(&mut self, _: u32, _: &Block) -> Result<(), DeviceError> {
        Err(DeviceError)
    }
}

struct IgnoredWrites(FileDevice);

impl Blocks for IgnoredWrites {
    fn read(&mut self, block: u32, data: &mut Block) -> Result<(), DeviceError> {
        self.0.read(block, data)
    }

    fn write(&mut self, _: u32, _: &Block) -> Result<(), DeviceError> {
        Ok(())
    }
}

#[test]
fn a_failed_or_lost_write_is_reported() {
    let fixture = make_fixture("failing", 1);
    let mut device = FailingWrites(load(&fixture));
    let volume = Volume::open(&mut device).unwrap();
    let file = volume.find(&mut device, SECOND_NAME).unwrap();
    assert_eq!(volume.overwrite(&mut device, &file, &[1, 2, 3]), Err(FatError::Device));

    let mut device = IgnoredWrites(load(&fixture));
    assert_eq!(volume.overwrite(&mut device, &file, &[1, 2, 3]), Err(FatError::Mismatch));
}
