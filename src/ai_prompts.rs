pub const REVIEW_PROMPT_VERSION: &str = "review_prompt_v2";
pub const COMPARE_PROMPT_VERSION: &str = "compare_prompt_v1";
pub const FOLLOW_UP_PROMPT_VERSION: &str = "follow_up_prompt_v1";

pub fn review_system_prompt() -> &'static str {
    include_str!("ai_prompts/review_prompt_v2.md")
}

pub fn compare_system_prompt() -> &'static str {
    include_str!("ai_prompts/compare_prompt_v1.md")
}

pub fn follow_up_system_prompt() -> &'static str {
    include_str!("ai_prompts/follow_up_prompt_v1.md")
}

pub fn review_task_framing() -> &'static str {
    include_str!("ai_prompts/review_task_frame_v2.md")
}

pub fn compare_task_framing() -> &'static str {
    include_str!("ai_prompts/compare_task_frame_v1.md")
}

pub fn follow_up_task_framing() -> &'static str {
    include_str!("ai_prompts/follow_up_task_frame_v1.md")
}
