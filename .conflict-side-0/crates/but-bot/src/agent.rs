pub enum AgentGraphNode {
    Route,
    CreateTodos,
    ExecuteTodo,
    Done(String),
}

pub struct AgentGraph {
    current_node: AgentGraphNode,
}

impl Default for AgentGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentGraph {
    pub fn new() -> Self {
        Self {
            current_node: AgentGraphNode::Route,
        }
    }

    pub fn start(&mut self, agent: &mut dyn Agent) -> anyhow::Result<String> {
        loop {
            match self.current_node {
                AgentGraphNode::Route => {
                    self.current_node = agent.route()?;
                }
                AgentGraphNode::CreateTodos => {
                    self.current_node = agent.create_todos()?;
                }
                AgentGraphNode::ExecuteTodo => {
                    self.current_node = agent.execute_todo()?;
                }
                AgentGraphNode::Done(ref response) => {
                    return Ok(response.clone());
                }
            }
        }
    }
}

pub trait Agent {
    fn route(&mut self) -> anyhow::Result<AgentGraphNode>;
    fn create_todos(&mut self) -> anyhow::Result<AgentGraphNode>;
    fn execute_todo(&mut self) -> anyhow::Result<AgentGraphNode>;
}
