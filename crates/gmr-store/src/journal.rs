use async_trait::async_trait;
use gmr_core::{AnchorKey, Entry, Seq};

use crate::error::StoreError;

/// 写入令牌。
///
/// 租约到期不等于持有者停工，所以日志必须能拒绝过期令牌的写入。`Held` 是
/// 一次租约签发的 epoch；`Unleased` 是没有租约的部署——那里本来就没有第二
/// 个写者可言。
///
/// 用枚举而不是「0 表示没有」：带内哨兵值会让调用方把「我没令牌」和「我
/// 的令牌是 0」混成一件事，而这两者该不该被拒是相反的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fence {
    Unleased,
    Held(u64),
}

impl Fence {
    pub fn epoch(self) -> Option<u64> {
        match self {
            Self::Unleased => None,
            Self::Held(n) => Some(n),
        }
    }
}

#[async_trait]
pub trait Journal: Send + Sync {
    async fn append(
        &self,
        anchor: &AnchorKey,
        entry: &Entry,
        fence: Fence,
    ) -> Result<Seq, StoreError>;

    async fn entries(&self, anchor: &AnchorKey, from: Seq)
    -> Result<Vec<(Seq, Entry)>, StoreError>;

    async fn anchors(&self) -> Result<Vec<AnchorKey>, StoreError>;
}

/// 令牌校验。两个后端共用这一份 —— 分头写迟早分头错。
pub fn guard(fence: Fence, seen: i64, entry: &Entry) -> Result<(), StoreError> {
    match fence {
        Fence::Held(epoch) if (epoch as i64) < seen => Err(StoreError::constraint(format!(
            "fencing 令牌 {epoch} 已过期（已见 {seen}）—— 租约到期不等于持有者停工"
        ))),
        // 观测是租约在管的活。这个锚一旦交给了租约，就不许再从旁边塞一条
        // 观测进来 —— 那正是租约要防的第二个写者。作者的修订不受此限。
        Fence::Unleased if seen > 0 && entry.is_sighting() => Err(StoreError::constraint(
            "这个锚的观测由租约在管，不接受没有令牌的观测；走队列，或者停掉轮询".to_owned(),
        )),
        _ => Ok(()),
    }
}
