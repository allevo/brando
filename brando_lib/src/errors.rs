
#[derive(PartialEq, Debug)]
pub enum AddBuildingError {
    InsufficientBudget,
    OutOfMap,
    AlreadyTaken,
}

#[derive(PartialEq, Debug)]
pub enum DeleteBuildingError {
    NoBuildingFound,
    OutOfMap,
}