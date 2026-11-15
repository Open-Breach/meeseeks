//! Which DRM format modifiers give a buffer more planes than its pixel
//! format has. Knows nothing about GTK or GStreamer.
//!
//! A modifier says how a graphics card lays a buffer out in memory. Some
//! compress the picture, and keep what that takes, or the colour the buffer
//! was cleared to, in planes of their own next to the picture. A frame in
//! such a buffer cannot be shown here. `pipewiresrc` describes a frame by
//! the planes of its pixel format and no more, and it leaves out a plane the
//! compositor gives no size for, which is what KWin does with these. GTK is
//! then handed a part of a buffer under the name of the whole, cannot import
//! it with any renderer, and the window stays grey.
//!
//! KWin takes such a modifier where it is offered one: on an Intel card it
//! is the first choice. Mutter offers only modifiers with a single plane. So
//! these are kept out of what the stream is asked for, see `showable` in
//! `src/display.rs`, and the compositor picks among the others.
//!
//! The modifiers are the ones the kernel's `drm_fourcc.h` says it of. One
//! that is not known here is taken to have no plane of its own.

/// The vendor a modifier belongs to is in its top 8 bits.
const VENDOR_SHIFT: u32 = 56;
const VENDOR_INTEL: u64 = 0x01;
const VENDOR_AMD: u64 = 0x02;

/// What is left of a modifier without the vendor.
const VALUE_MASK: u64 = (1 << VENDOR_SHIFT) - 1;

/// AMD's bit for delta colour compression. Its data is in plane 1, and with
/// `DCC_RETILE` there is another in plane 2.
const AMD_DCC: u64 = 1 << 13;

/// Whether a buffer with this modifier has planes that its pixel format has
/// not.
pub fn has_planes_of_its_own(modifier: u64) -> bool {
    let value = modifier & VALUE_MASK;
    match modifier >> VENDOR_SHIFT {
        // The modifiers with a colour control surface, CCS, in a plane of its
        // own, which are 4 to 8, 13 and 14, and those with the clear colour
        // in one, which are 8, 12 and 15. Not 10, 11, 16 and 17: they
        // compress as well, but keep the CCS outside the buffer.
        VENDOR_INTEL => matches!(value, 4..=8 | 12..=15),
        VENDOR_AMD => value & AMD_DCC != 0,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn intel(value: u64) -> u64 {
        VENDOR_INTEL << VENDOR_SHIFT | value
    }

    const fn amd(value: u64) -> u64 {
        VENDOR_AMD << VENDOR_SHIFT | value
    }

    /// The three KWin chooses from on an Intel Arrow Lake, by what the
    /// driver there answers `gbm_device_get_format_modifier_plane_count` for
    /// ARGB8888: 2, 2 and 3 planes.
    #[test]
    fn compressed_on_a_recent_intel_card() {
        assert!(has_planes_of_its_own(0x0100_0000_0000_000d));
        assert!(has_planes_of_its_own(0x0100_0000_0000_000e));
        assert!(has_planes_of_its_own(0x0100_0000_0000_000f));
    }

    #[test]
    fn compressed_on_an_older_intel_card() {
        for value in [4, 5, 6, 7, 8, 12] {
            assert!(has_planes_of_its_own(intel(value)), "{value}");
        }
    }

    /// Linear, the tiled ones, and the compressed ones that keep the CCS
    /// outside the buffer.
    #[test]
    fn one_plane_on_an_intel_card() {
        assert!(!has_planes_of_its_own(0));
        for value in [1, 2, 3, 9, 10, 11, 16, 17] {
            assert!(!has_planes_of_its_own(intel(value)), "{value}");
        }
    }

    #[test]
    fn compressed_on_an_amd_card() {
        let tiled = amd(0x0904);
        assert!(!has_planes_of_its_own(tiled));
        assert!(has_planes_of_its_own(tiled | AMD_DCC));
        assert!(has_planes_of_its_own(tiled | AMD_DCC | 1 << 14));
    }

    #[test]
    fn of_a_vendor_not_known_here() {
        assert!(!has_planes_of_its_own(0x0300_0000_0060_6014));
        assert!(!has_planes_of_its_own(0x0800_0000_0000_0041));
    }
}
