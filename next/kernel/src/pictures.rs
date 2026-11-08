//! Which pictures appear where, and the keys that change them.

use crate::assets::{self, Assets};
use crate::board;
use crate::image;
use crate::spin::EXTRA_ROWS;
use crate::video::{Framebuffer, BLACK};

// Where things sit on the screen. Aki is taller than the screen, so she's clipped. The
// logo hovers over her outstretched hand, whose palm is centered near x 1250 with its
// top near y 430, and touches none of her pixels, so neither covers the other.
pub const LOGO_LEFT: usize = 1166;
pub const LOGO_TOP: usize = 144;
/// `logo.png`'s size, which `scripts/make-assets.py` keeps.
pub const LOGO_WIDTH: usize = 168;
pub const LOGO_HEIGHT: usize = 256;
pub const LOGO_CENTER_X: usize = LOGO_LEFT + LOGO_WIDTH / 2;
pub const LOGO_CENTER_Y: usize = LOGO_TOP + LOGO_HEIGHT / 2;
const AKI_LEFT: usize = 1152;
const AKI_TOP: usize = 0;
const MICROSECONDS_PER_MILLISECOND: u32 = 1000;

pub const CLEAR_KEY: u8 = b'0';
/// Key `1` shows the first version of Aki, and so on.
const FIRST_AKI_KEY: u8 = b'1';
/// The version on screen at startup: the original, at full color.
const STARTUP_AKI: usize = 0;

/// A version of Aki. Its mouth's position is relative to her, and differs between
/// versions because a mouth is cut out along whole blocks. `scripts/make-assets.py`
/// must agree, which `scripts/test_pc98.py` checks.
struct AkiVersion {
    body: usize,
    mouth: usize,
    mouth_left: usize,
    mouth_top: usize,
}

const AKI_VERSIONS: [AkiVersion; 4] = [
    AkiVersion { body: assets::AKI_ORIGINAL, mouth: assets::AKI_ORIGINAL_MOUTH, mouth_left: 368, mouth_top: 220 },
    AkiVersion { body: assets::AKI, mouth: assets::AKI_MOUTH, mouth_left: 368, mouth_top: 220 },
    AkiVersion { body: assets::AKI_X2, mouth: assets::AKI_X2_MOUTH, mouth_left: 368, mouth_top: 220 },
    AkiVersion { body: assets::AKI_X4, mouth: assets::AKI_X4_MOUTH, mouth_left: 368, mouth_top: 220 },
];

fn draw(framebuffer: &Framebuffer, assets: &mut Assets, name: &str, slot: usize, left: usize, top: usize) {
    let drawn = assets.get(slot).map(|data| image::draw(framebuffer, left, top, data));
    if !matches!(drawn, Some(Ok(()))) {
        println!("Couldn't draw {name}.");
    }
}

fn draw_aki(framebuffer: &Framebuffer, assets: &mut Assets, version: &AkiVersion) {
    draw(framebuffer, assets, "Aki", version.body, AKI_LEFT, AKI_TOP);
    let (left, top) = (AKI_LEFT + version.mouth_left, AKI_TOP + version.mouth_top);
    draw(framebuffer, assets, "Aki's mouth", version.mouth, left, top);
}

/// Draws what the screen starts with: the logo, and the startup version of Aki.
pub fn draw_startup(framebuffer: &Framebuffer, assets: &mut Assets) {
    let start = board::microseconds();
    draw(framebuffer, assets, "the logo", assets::LOGO, LOGO_LEFT, LOGO_TOP);
    draw_aki(framebuffer, assets, &AKI_VERSIONS[STARTUP_AKI]);
    let elapsed = board::microseconds().wrapping_sub(start);
    println!("Images drawn in {} ms.", elapsed / MICROSECONDS_PER_MILLISECOND);
}

/// Blanks Aki's side of the screen, apart from the logo's box and the rows its turning
/// reaches, where a cube can be too. Aki has no pixels there, so none of a bigger
/// version's edges can be left around a smaller one.
fn clear_aki(framebuffer: &Framebuffer) {
    let width = framebuffer.width.saturating_sub(AKI_LEFT);
    let height = framebuffer.height;
    let (logo_top, logo_bottom) = (LOGO_TOP - EXTRA_ROWS, LOGO_TOP + LOGO_HEIGHT + EXTRA_ROWS);
    let logo_right = LOGO_LEFT + LOGO_WIDTH;
    framebuffer.fill_rect(AKI_LEFT, 0, LOGO_LEFT - AKI_LEFT, height, BLACK);
    framebuffer.fill_rect(logo_right, 0, width - (logo_right - AKI_LEFT), height, BLACK);
    framebuffer.fill_rect(LOGO_LEFT, 0, LOGO_WIDTH, logo_top, BLACK);
    framebuffer.fill_rect(LOGO_LEFT, logo_bottom, LOGO_WIDTH, height - logo_bottom, BLACK);
}

// The logo's box, with its reach, sits inside Aki's side and the screen.
const _: () = assert!(AKI_LEFT <= LOGO_LEFT && LOGO_TOP >= EXTRA_ROWS);

/// `0` clears the screen, and `1` onward draw a version of Aki in place of what's there.
/// Other keys do nothing. Serial input isn't read during this, so anything sent
/// while reading or drawing beyond what the UART holds is lost.
pub fn handle_key(framebuffer: &Framebuffer, assets: &mut Option<Assets>, key: u8) {
    if key == CLEAR_KEY {
        framebuffer.fill(BLACK);
        return;
    }
    let Some(version) = key.checked_sub(FIRST_AKI_KEY).and_then(|index| AKI_VERSIONS.get(usize::from(index))) else {
        return;
    };
    if let Some(assets) = assets {
        // Read first, which can take over 100 ms from the card, so the blank is brief.
        let _ = assets.get(version.body);
        let _ = assets.get(version.mouth);
        clear_aki(framebuffer);
        draw_aki(framebuffer, assets, version);
    }
}
