use std::collections::HashMap;

use anyhow::Result;
use serde::Deserialize;

use crate::contracts::{TaskTypeInfo, TaskTypeInput};
use crate::db::Database;

#[derive(Debug, Clone, Deserialize)]
pub struct TaskTypeDefinition {
    pub name: String,
    pub description: String,
    pub inputs: Vec<InputDef>,
    pub stages: Vec<StageDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InputDef {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub required: bool,
    pub default: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StageDef {
    pub name: String,
    pub agent: String,
    #[serde(default)]
    pub needs_branch: bool,
    pub prompt: String,
    pub context_from: Option<String>,
    pub on_fail: Option<OnFailAction>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OnFailAction {
    pub goto: String,
    pub carry: String,
    pub max_retries: i32,
}

/// 从 DB 读取模板并解析为 TaskTypeDefinition
pub async fn get_task_type_from_db(db: &Database, name: &str) -> Result<Option<TaskTypeDefinition>> {
    let template = db.get_template_by_name(name).await?;
    match template {
        Some(t) => {
            let def: TaskTypeDefinition = serde_yaml::from_str(&t.definition)?;
            Ok(Some(def))
        }
        None => Ok(None),
    }
}

/// 从 DB 读取所有模板信息（用于 API 列表）
pub async fn list_task_types_from_db(db: &Database) -> Result<Vec<TaskTypeInfo>> {
    let templates = db.list_templates().await?;
    let mut infos = Vec::new();
    for t in templates {
        if let Ok(def) = serde_yaml::from_str::<TaskTypeDefinition>(&t.definition) {
            infos.push(def.to_info());
        }
    }
    Ok(infos)
}

/// 变量替换：`${var}` → value
pub fn substitute(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("${{{key}}}"), value);
    }
    result
}

impl TaskTypeDefinition {
    /// 转换为 API 响应类型
    pub fn to_info(&self) -> TaskTypeInfo {
        TaskTypeInfo {
            name: self.name.clone(),
            description: self.description.clone(),
            inputs: self
                .inputs
                .iter()
                .map(|i| TaskTypeInput {
                    name: i.name.clone(),
                    description: i.description.clone(),
                    required: i.required,
                    default: i.default.clone(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_stage_yaml() {
        let yaml = include_str!("../../task-types/single-stage.yaml");
        let def: TaskTypeDefinition = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.stages.len(), 1);
        assert_eq!(def.stages[0].name, "coding");
        assert!(def.stages[0].needs_branch);
        assert_eq!(def.inputs.len(), 3);
        assert!(def.inputs[0].required);
    }

    #[test]
    fn parse_feature_dev_yaml() {
        let yaml = include_str!("../../task-types/feature-dev.yaml");
        let def: TaskTypeDefinition = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.stages.len(), 2);
        assert_eq!(def.stages[1].name, "testing");
        assert_eq!(def.stages[1].context_from.as_deref(), Some("coding"));
        let on_fail = def.stages[1].on_fail.as_ref().unwrap();
        assert_eq!(on_fail.goto, "coding");
        assert_eq!(on_fail.carry, "error_report");
        assert_eq!(on_fail.max_retries, 3);
    }

    #[test]
    fn substitute_variables() {
        let mut vars = HashMap::new();
        vars.insert("agent_type".into(), "claude-code".into());
        vars.insert("prompt".into(), "build a calculator".into());
        let result = substitute("Use ${agent_type} to ${prompt}", &vars);
        assert_eq!(result, "Use claude-code to build a calculator");
    }

    #[test]
    fn substitute_missing_variable_left_as_is() {
        let vars = HashMap::new();
        let result = substitute("Hello ${name}", &vars);
        assert_eq!(result, "Hello ${name}");
    }

    #[tokio::test]
    async fn get_task_type_from_db_works() {
        let db = Database::new(":memory:").await.unwrap();
        db.migrate().await.unwrap();
        db.seed_builtin_templates().await.unwrap();

        let ss = get_task_type_from_db(&db, "single-stage").await.unwrap();
        assert!(ss.is_some());
        assert_eq!(ss.unwrap().stages.len(), 1);

        let fd = get_task_type_from_db(&db, "feature-dev").await.unwrap();
        assert!(fd.is_some());
        assert_eq!(fd.unwrap().stages.len(), 2);

        let missing = get_task_type_from_db(&db, "nonexistent").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn list_task_types_from_db_works() {
        let db = Database::new(":memory:").await.unwrap();
        db.migrate().await.unwrap();
        db.seed_builtin_templates().await.unwrap();

        let infos = list_task_types_from_db(&db).await.unwrap();
        assert_eq!(infos.len(), 2);
    }
}
