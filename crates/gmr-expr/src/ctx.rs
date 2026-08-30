use serde_json::Value;

#[derive(Debug, Clone, Copy)]
pub struct Ctx<'a> {
    pub obs: &'a Value,
    pub state: &'a Value,
    pub taken_at: i64,
    pub entered_at: i64,
    pub anchors: &'a [Value],
}

impl<'a> Ctx<'a> {
    pub fn new(obs: &'a Value, state: &'a Value) -> Self {
        Self {
            obs,
            state,
            taken_at: 0,
            entered_at: 0,
            anchors: &[],
        }
    }

    pub fn at(mut self, taken_at: i64, entered_at: i64) -> Self {
        self.taken_at = taken_at;
        self.entered_at = entered_at;
        self
    }

    pub fn over(mut self, anchors: &'a [Value]) -> Self {
        self.anchors = anchors;
        self
    }

    pub(crate) fn each(&self, state: &'a Value) -> Self {
        Self { state, ..*self }
    }
}
