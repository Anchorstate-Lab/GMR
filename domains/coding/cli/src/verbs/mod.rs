pub mod bind;
pub mod close;
pub mod doctor;
pub mod edges;
pub mod observe;
pub mod open;
pub mod pass;
pub mod publish;
pub mod read;
pub mod reprobe;
pub mod restate;
pub mod reterminal;
pub mod retransition;
pub mod sync;

use gmr::ContentHash;

pub(crate) fn sealed(context: &ContentHash, rationale: &ContentHash) {
    println!(
        "  context   {} （基底捕获，不可能被伪造）",
        &context.as_str()[..12]
    );
    println!(
        "  rationale {} （你写的，基底不读、只担保不可篡改）",
        &rationale.as_str()[..12]
    );
}
