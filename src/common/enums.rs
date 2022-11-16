#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EducationLevel {
    None,
    #[allow(dead_code)]
    Low,
}

impl PartialOrd for EducationLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (EducationLevel::None, EducationLevel::None)
            | (EducationLevel::Low, EducationLevel::Low) => Some(std::cmp::Ordering::Equal),
            (_, EducationLevel::Low) => Some(std::cmp::Ordering::Less),
            (EducationLevel::Low, _) => Some(std::cmp::Ordering::Greater),
        }
    }
}
