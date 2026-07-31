use serde_json::Value;

#[derive(Debug, Clone, Copy)]
pub struct Ctx<'a> {
    pub obs: &'a Value,
    pub state: &'a Value,
    pub taken_at: i64,
    pub entered_at: i64,
}

impl<'a> Ctx<'a> {
    pub fn new(obs: &'a Value, state: &'a Value) -> Self {
        Self {
            obs,
            state,
            taken_at: 0,
            entered_at: 0,
        }
    }

    pub fn at(mut self, taken_at: i64, entered_at: i64) -> Self {
        self.taken_at = taken_at;
        self.entered_at = entered_at;
        self
    }

    pub(crate) fn changed(&self, direction: &str) -> bool {
        let now = self.obs.get(direction);
        let was = self.state.get(direction);
        now != was
    }
}
