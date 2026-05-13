//! One-shot icon generator for Abyss Singularity.
//!
//! Run: `cargo run --bin icon-gen` (from `src-tauri/`)
//!
//! Concept: a literal cosmic singularity. Deep navy void → soft outer
//! atmospheric bloom → asymmetric glowing event-horizon ring → blinding
//! white-cyan core. The ring is offset from perfect circular symmetry to
//! suggest accretion-disc rotation; the inner halo has a slight angular
//! falloff for a sense of motion.
//!
//! Writes all sizes Tauri's NSIS bundle wants into `src-tauri/icons/`,
//! plus an `icon.ico` multi-image bundle for the Windows binary.

use std::f32::consts::TAU;
use std::path::PathBuf;

use image::{ImageBuffer, Rgba, RgbaImage};

const MASTER: u32 = 1024;
const PNG_SIZES: &[u32] = &[
    32, 64, 96, 128, 256, 512, 1024,
];
const ICO_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out_dir = manifest_dir.join("icons");
    std::fs::create_dir_all(&out_dir)?;

    // Render once at master resolution, then downscale via Lanczos3 for
    // every other size — single source of truth, no per-size hand-tuning.
    println!("rendering master at {MASTER}x{MASTER}...");
    let master = render(MASTER);
    master.save(out_dir.join("master.png"))?;

    for &size in PNG_SIZES {
        let img = if size == MASTER {
            master.clone()
        } else {
            image::imageops::resize(&master, size, size, image::imageops::FilterType::Lanczos3)
        };
        let name = match size {
            32  => "32x32.png".to_string(),
            128 => "128x128.png".to_string(),
            256 => "128x128@2x.png".to_string(), // Tauri convention
            other => format!("{other}x{other}.png"),
        };
        img.save(out_dir.join(&name))?;
        println!("  wrote {}", name);
    }

    // Replace the generic icon.png with the 512px variant.
    let icon512 = image::imageops::resize(&master, 512, 512, image::imageops::FilterType::Lanczos3);
    icon512.save(out_dir.join("icon.png"))?;
    println!("  wrote icon.png (512)");

    // Bundle a multi-resolution ICO so the Windows shell picks the right
    // size at every zoom level (taskbar 24px, alt-tab 32px, explorer 48px,
    // jumbo 256px).
    let mut bundle = ico::IconDir::new(ico::ResourceType::Icon);
    for &size in ICO_SIZES {
        let img = image::imageops::resize(&master, size, size, image::imageops::FilterType::Lanczos3);
        let rgba = img.into_raw();
        let entry = ico::IconImage::from_rgba_data(size, size, rgba);
        bundle.add_entry(ico::IconDirEntry::encode(&entry)?);
    }
    let ico_path = out_dir.join("icon.ico");
    let mut file = std::fs::File::create(&ico_path)?;
    bundle.write(&mut file)?;
    println!("  wrote icon.ico ({} entries)", ICO_SIZES.len());

    Ok(())
}

/// Render the master Abyss Singularity at the given square size.
fn render(size: u32) -> RgbaImage {
    let s = size as f32;
    let cx = s / 2.0;
    let cy = s / 2.0;
    // Outer extent we draw to — leaves a hint of padding so OS icon
    // composites don't clip the bloom.
    let max_r = s * 0.49;

    // Asymmetric ring: a touch off-centre so the icon has a sense of
    // motion / rotation instead of feeling like a flat target.
    let ring_offset_x = -s * 0.012;
    let ring_offset_y = -s * 0.018;
    let ring_radius   = max_r * 0.62;
    let ring_thickness = max_r * 0.058;

    let mut img: RgbaImage = ImageBuffer::new(size, size);
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let r  = (dx * dx + dy * dy).sqrt();
            let nr = r / max_r;            // 0..1+ from centre to extent
            let theta = dy.atan2(dx);

            // ---- background void --------------------------------------
            // Deep navy with a barely-there radial gradient so the icon
            // reads as a sphere rather than a flat disc.
            let base = lerp_color([3.0, 6.0, 11.0], [9.0, 14.0, 22.0], (1.0 - nr).clamp(0.0, 1.0));

            // ---- outer atmospheric bloom -----------------------------
            // Cyan haze that falls off softly past the ring. Adds the
            // sense of accretion-disc light leaking into space.
            let bloom = (-((nr - 0.7).max(0.0).powi(2)) * 8.0).exp() * 35.0 * (1.0 - nr).clamp(0.0, 1.0);

            // ---- event horizon ring ----------------------------------
            let rdx = x as f32 - cx - ring_offset_x;
            let rdy = y as f32 - cy - ring_offset_y;
            let ring_r = (rdx * rdx + rdy * rdy).sqrt();
            let ring_d = (ring_r - ring_radius).abs();
            // Two-band ring: a tight bright core + a wide soft halo.
            let core_band = (-((ring_d / ring_thickness).powi(2)) * 4.0).exp();
            let halo_band = (-((ring_d / (ring_thickness * 3.5)).powi(2)) * 1.2).exp();
            let ring = core_band * 280.0 + halo_band * 90.0;

            // Slight angular modulation — brighter on one side of the
            // ring to suggest a hotspot in the disc.
            let hotspot = (theta - 0.7).cos().max(0.0).powf(3.0);
            let ring   = ring * (0.75 + 0.45 * hotspot);

            // ---- inner darkening (gravitational shadow) --------------
            // Region just inside the ring is darker than the rest of the
            // void — like looking down into the well. We blend it under
            // everything else.
            let inside_factor = ((ring_radius - ring_r) / (ring_radius * 0.7))
                .clamp(0.0, 1.0)
                .powf(1.4);
            let inside_darken = 1.0 - inside_factor * 0.55;

            // ---- the singularity itself ------------------------------
            // A blindingly bright point right at the centre, hot-white
            // fading to cyan as it fans out.
            let core_glow_tight = (-(nr * nr) * 220.0).exp() * 255.0;
            let core_glow_wide  = (-(nr * nr) * 18.0).exp()  * 90.0;
            let singularity = core_glow_tight + core_glow_wide;

            // ---- compose ----------------------------------------------
            let mut rch = base[0] * inside_darken;
            let mut gch = base[1] * inside_darken;
            let mut bch = base[2] * inside_darken;

            // Cyan light: heavy blue, strong green, mild red. (R, G, B
            // weights below are tuned to the abyss palette accent.)
            rch += bloom * 0.10 + ring * 0.18 + singularity * 0.85;
            gch += bloom * 0.65 + ring * 0.85 + singularity * 0.95;
            bch += bloom * 0.90 + ring * 1.00 + singularity * 1.00;

            // The colour itself is already vignetted via radial darkening,
            // so we keep alpha uniformly opaque — icons stay crisp at small
            // sizes instead of dissolving at the corners.
            img.put_pixel(x, y, Rgba([
                clamp_u8(rch),
                clamp_u8(gch),
                clamp_u8(bch),
                255,
            ]));
        }
    }
    img
}

fn lerp_color(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[inline]
fn clamp_u8(v: f32) -> u8 {
    v.clamp(0.0, 255.0) as u8
}

// Silence unused-warning in the off chance `TAU` migration removes it.
#[allow(dead_code)]
fn _unused() { let _ = TAU; }
