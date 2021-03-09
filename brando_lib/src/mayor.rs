#[derive(Debug)]
pub enum DescrementBudgetError {}

pub trait Mayor {
    fn has_budget(self: &Self, cost: u32) -> bool;

    fn decrement_budget(self: &mut Self, cost: u32) -> Result<(), DescrementBudgetError>;
}

pub struct MainMayor {}

impl MainMayor {
    pub fn new() -> Self {
        Self {}
    }
}

impl Mayor for MainMayor {
    fn has_budget(self: &Self, cost: u32) -> bool {
        true
    }

    fn decrement_budget(self: &mut Self, cost: u32) -> Result<(), DescrementBudgetError> {
        Ok(())
    }
}
