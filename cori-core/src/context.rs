/// Session 04 · Context & Token Management
///
/// 问题：AgentLoop 的 messages 每轮都在增长，token 消耗不断累积。
/// 长任务最终会撞上模型的 context window 上限，导致请求失败。
///
/// 这一节实现一个简单但有效的截断策略：
///   保留第 0 条消息（原始用户请求）+ 最近 N 条消息

use crate::types::Message;

// ── ContextManager ────────────────────────────────────────────────────────────

pub struct ContextManager {
    /// 触发截断的 input token 阈值
    /// 超过这个值时，在下次发送前截断 messages
    pub token_threshold: u32,
    /// 截断后最多保留多少条消息（不含第 0 条）
    pub keep_last: usize,
}

impl ContextManager {
    pub fn new(token_threshold: u32, keep_last: usize) -> Self {
        Self { token_threshold, keep_last }
    }

    /// 默认配置：80k token 触发截断，保留最近 20 条
    pub fn default_config() -> Self {
        Self::new(80_000, 20)
    }

    /// 判断是否需要截断
    pub fn should_truncate(&self, input_tokens: u32) -> bool {
        input_tokens >= self.token_threshold
    }

    /// 截断 messages，保留第 0 条 + 最后 `keep_last` 条。
    ///
    /// Exercise 1：补全这个方法。
    ///
    /// 为什么一定要保留第 0 条？
    ///   messages[0] 是用户的原始请求。丢掉它，Claude 就不知道自己在做什么任务了。
    ///
    /// 为什么从中间截，而不是从末尾截？
    ///   最近的消息包含最新的工具执行结果，是 Claude 做决策最需要的上下文。
    ///   旧的中间消息（早期的工具调用记录）价值最低，优先丢弃。
    ///
    /// 截断示意：
    ///   before: [msg0, msg1, msg2, msg3, msg4, msg5, msg6]  keep_last=3
    ///   after:  [msg0, msg4, msg5, msg6]
    pub fn truncate(&self, messages: &mut Vec<Message>) {
        // TODO：
        // 条件：messages.len() > 1 + self.keep_last 时才需要截断
        // 操作：保留 messages[0]，再保留末尾 keep_last 条
        //
        // 提示：
        //   let tail = messages.split_off(messages.len() - self.keep_last);
        //   messages.truncate(1);
        //   messages.extend(tail);
        todo!("实现截断逻辑")
    }
}

// ── 测试 ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Message;

    fn msgs(n: usize) -> Vec<Message> {
        (0..n).map(|i| Message::user(format!("msg {i}"))).collect()
    }

    /// 未达到阈值时不截断
    #[test]
    fn test_no_truncate_below_threshold() {
        let ctx = ContextManager::new(1000, 3);
        assert!(!ctx.should_truncate(999));
    }

    /// 达到阈值时判断需要截断
    #[test]
    fn test_should_truncate_at_threshold() {
        let ctx = ContextManager::new(1000, 3);
        assert!(ctx.should_truncate(1000));
    }

    /// 截断后：第 0 条保留，末尾 keep_last 条保留，中间丢弃
    #[test]
    fn test_truncate_keeps_first_and_last() {
        let ctx = ContextManager::new(1000, 3);
        let mut messages = msgs(7); // msg0 ~ msg6

        ctx.truncate(&mut messages);

        assert_eq!(messages.len(), 4); // msg0 + msg4,5,6
        // 第 0 条还是原始的 msg0
        if let crate::types::Content::Text { text } = &messages[0].content[0] {
            assert_eq!(text, "msg 0");
        }
        // 最后一条是 msg6
        if let crate::types::Content::Text { text } = &messages[3].content[0] {
            assert_eq!(text, "msg 6");
        }
    }

    /// messages 数量不足时不应 panic
    #[test]
    fn test_truncate_small_messages_no_panic() {
        let ctx = ContextManager::new(1000, 3);
        let mut messages = msgs(2);
        ctx.truncate(&mut messages); // 不应 panic，也不应改变内容
        assert_eq!(messages.len(), 2);
    }
}
