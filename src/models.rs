use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct HASSApiBody {
    pub entity_id: String,
    pub rgb_color: [u32; 3],
    pub brightness: u32,
}
