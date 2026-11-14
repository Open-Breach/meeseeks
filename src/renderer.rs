//! Which of GTK's renderers draws the window. GTK takes the choice from its
//! environment, so it has to be made before GTK is initialised.

/// The renderers there is a choice between.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Renderer {
    #[default]
    Gl,
    Vulkan,
    Cairo,
}

impl Renderer {
    pub fn label(self) -> &'static str {
        match self {
            Self::Gl => "GL",
            Self::Vulkan => "Vulkan",
            Self::Cairo => "Cairo",
        }
    }

    /// The renderer a `GSK_RENDERER` value names, if it is one of these.
    fn from_gsk_name(name: &str) -> Option<Self> {
        match name {
            "gl" | "ngl" | "opengl" => Some(Self::Gl),
            "vulkan" => Some(Self::Vulkan),
            "cairo" => Some(Self::Cairo),
            _ => None,
        }
    }

    /// The renderer behind the type name of a `GskRenderer`.
    pub fn from_type_name(name: &str) -> Option<Self> {
        match name {
            "GskGLRenderer" | "GskNglRenderer" => Some(Self::Gl),
            "GskVulkanRenderer" => Some(Self::Vulkan),
            "GskCairoRenderer" => Some(Self::Cairo),
            _ => None,
        }
    }
}

/// Asks GTK for `renderer`, unless `GSK_RENDERER` is set: the environment
/// wins. Returns what GTK has been asked for, which is `None` if
/// `GSK_RENDERER` names something other than the three above.
///
/// GL is asked for by keeping GTK off Vulkan, its default on Wayland, rather
/// than by name, because the name of the GL renderer differs between GTK
/// versions. Staying off Vulkan went in as a cure for the stutter, which it
/// is not: sessions have stuttered with either renderer. It is the default
/// for what it does do, on the one GNOME machine this has run on, a laptop
/// with a second GPU: GTK no longer wakes that GPU when the app starts, and
/// the picture comes up as soon as the portal has answered instead of seconds
/// later.
///
/// # Safety
///
/// This changes the environment, so it has to run before any other thread
/// exists that could be reading it: first thing in `main`.
pub unsafe fn request(renderer: Renderer) -> Option<Renderer> {
    if let Some(name) = std::env::var_os("GSK_RENDERER") {
        return Renderer::from_gsk_name(&name.to_string_lossy());
    }
    match renderer {
        Renderer::Gl => {
            let mut disabled = std::env::var("GDK_DISABLE").unwrap_or_default();
            if !disabled.is_empty() {
                disabled.push(',');
            }
            disabled.push_str("vulkan");
            // SAFETY: see above; the caller vouches for it.
            unsafe { std::env::set_var("GDK_DISABLE", disabled) };
        }
        // SAFETY: as above.
        Renderer::Vulkan => unsafe { std::env::set_var("GSK_RENDERER", "vulkan") },
        // SAFETY: as above.
        Renderer::Cairo => unsafe { std::env::set_var("GSK_RENDERER", "cairo") },
    }
    Some(renderer)
}
