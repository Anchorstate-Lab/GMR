use gmr_core::AnchorKey;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("锚 `{key}` 还没开过")]
    NoSuchAnchor { key: AnchorKey },

    #[error("锚 `{key}` 已经开过了 —— 要改它请用 revise，那会留下一条密封的记录")]
    AlreadyOpen { key: AnchorKey },

    #[error(
        "锚 `{key}` 已经终结了 —— 终结不可撤销。\
         判据写错了要开新的一代（supersede `{key}`），不是把这一个拽回来"
    )]
    AnchorClosed { key: AnchorKey },

    #[error("`{key}` 还开着 —— 只有终结了的锚才谈得上被接替")]
    NotClosedYet { key: AnchorKey },

    #[error(
        "开锚时探针跑不起来：{message}。\
         锚可以先于它的目标存在，但一次都观测不成就没有起点可捕获"
    )]
    CannotOpen { message: String },

    #[error("这个部署没有配置 Queue —— pass 是仅轮询部署的动词")]
    NoQueue,

    #[error("`{key}` 的租约正被别人持着 —— 让持有者写完，别从旁边塞一条进去")]
    Leased { key: AnchorKey },

    #[error(transparent)]
    Store(#[from] gmr_store::StoreError),
}
