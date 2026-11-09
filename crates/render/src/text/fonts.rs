//! Bundled fonts. Only these are used — never system fonts — so renders are
//! reproducible across machines (golden tests).

/// One font face of the library.
#[derive(Debug, Clone, Copy)]
pub struct FontFace {
    pub family: &'static str,
    pub weight: u16,
    pub data: &'static [u8],
}

/// Bundled OFL fonts (`assets/fonts/`, Inter 4.1).
pub const BUNDLED: &[FontFace] = &[
    FontFace {
        family: "Inter",
        weight: 400,
        data: include_bytes!("../../../../assets/fonts/Inter-Regular.ttf"),
    },
    FontFace {
        family: "Inter",
        weight: 600,
        data: include_bytes!("../../../../assets/fonts/Inter-SemiBold.ttf"),
    },
    FontFace {
        family: "Inter Display",
        weight: 700,
        data: include_bytes!("../../../../assets/fonts/InterDisplay-Bold.ttf"),
    },
    FontFace {
        family: "Inter Display",
        weight: 900,
        data: include_bytes!("../../../../assets/fonts/InterDisplay-Black.ttf"),
    },
];

/// The set of fonts available to text engines and the outline renderer.
#[derive(Debug, Clone)]
pub struct FontLibrary {
    faces: Vec<FontFace>,
}

impl Default for FontLibrary {
    fn default() -> Self {
        Self::bundled()
    }
}

impl FontLibrary {
    pub fn bundled() -> Self {
        Self {
            faces: BUNDLED.to_vec(),
        }
    }

    pub fn faces(&self) -> &[FontFace] {
        &self.faces
    }

    pub fn data(&self, index: usize) -> &'static [u8] {
        self.faces[index].data
    }

    /// Finds the library index of a face by its raw data pointer.
    pub fn index_of_data(&self, data: &[u8]) -> Option<usize> {
        self.faces.iter().position(|f| {
            std::ptr::eq(f.data.as_ptr(), data.as_ptr()) && f.data.len() == data.len()
        })
    }
}
