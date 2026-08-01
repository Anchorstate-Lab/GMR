use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use gmr_core::AnchorKey;

use crate::error::StoreError;
use crate::journal::Fence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    pub anchor: AnchorKey,
    pub fence: Fence,
    pub lease_until: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Reschedule { after_secs: i64 },
    Backoff { after_secs: i64 },
    Retire,
}

/// 实现方必须保证：**同一个锚签发的 fence 严格单调递增，且退场不清零。**
/// 日志拿它当高水位来挡过期租约的写入，倒退一次就等于把那个锚永久锁死。
#[async_trait]
pub trait Queue: Send + Sync {
    async fn enqueue(&self, anchor: &AnchorKey, due: DateTime<Utc>) -> Result<(), StoreError>;

    async fn due(
        &self,
        now: DateTime<Utc>,
        lease: Duration,
        limit: usize,
    ) -> Result<Vec<Ticket>, StoreError>;

    /// 单独为一个锚取租约，不管它到没到点。
    ///
    /// 手工触发的观测走这里 —— 否则它只能绕过令牌去写，而那正是租约要防
    /// 的第二个写者。取不到说明别人正持着，那就该让别人写。
    async fn lease(
        &self,
        anchor: &AnchorKey,
        now: DateTime<Utc>,
        lease: Duration,
    ) -> Result<Option<Ticket>, StoreError>;

    async fn settle(
        &self,
        ticket: &Ticket,
        disposition: Disposition,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError>;
}
