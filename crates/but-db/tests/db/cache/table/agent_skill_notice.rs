use but_db::cache::AgentSkillNotice;
use chrono::DateTime;

use crate::cache::in_memory_cache;

#[test]
fn save_get_and_replace() -> anyhow::Result<()> {
    let mut cache = in_memory_cache();
    assert_eq!(cache.agent_skill_notice().try_get()?, None);

    let first = AgentSkillNotice {
        shown_at: DateTime::from_timestamp(1000000, 0).unwrap(),
    };
    cache.agent_skill_notice_mut()?.save(&first)?;
    assert_eq!(cache.agent_skill_notice().try_get()?, Some(first));

    let second = AgentSkillNotice {
        shown_at: DateTime::from_timestamp(2000000, 0).unwrap(),
    };
    cache.agent_skill_notice_mut()?.save(&second)?;
    assert_eq!(cache.agent_skill_notice().try_get()?, Some(second));

    Ok(())
}
