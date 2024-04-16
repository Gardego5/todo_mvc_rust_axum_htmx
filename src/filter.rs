use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub enum Filter {
    #[default]
    All,
    Active,
    Completed,
}

impl ToString for Filter {
    fn to_string(&self) -> String {
        ToString::to_string(match self {
            Filter::All => "All",
            Filter::Active => "Active",
            Filter::Completed => "Completed",
        })
    }
}
