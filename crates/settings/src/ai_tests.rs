use super::*;

fn put(c: &mut AiChoices, old: Option<&str>, name: &str, prompt: &str) -> Result<(), RecipeError> {
    c.edit(AiEdit::Put {
        old: old.map(str::to_owned),
        name: name.to_owned(),
        prompt: prompt.to_owned(),
    })
}

fn check(c: &mut AiChoices, id: &str, on: bool) {
    c.edit(AiEdit::Check {
        id: id.to_owned(),
        on,
    })
    .unwrap();
}

fn ids(c: &AiChoices) -> Vec<String> {
    c.checked_recipes().into_iter().map(|r| r.id).collect()
}

#[test]
fn the_list_is_the_built_in_recipes_then_the_own_ones() {
    let mut choices = AiChoices::default();
    assert_eq!(ids(&choices), ["pauses", "zooms", "captions"]);
    put(&mut choices, None, " Лого ", "Вставь лого в угол").unwrap();
    assert_eq!(ids(&choices), ["pauses", "zooms", "captions", "Лого"]);
    assert_eq!(
        choices.recipes().last().unwrap().prompt,
        "Вставь лого в угол"
    );
    check(&mut choices, "zooms", false);
    check(&mut choices, "slips", true);
    check(&mut choices, "нет такого", true);
    assert_eq!(
        ids(&choices),
        ["pauses", "captions", "slips", "Лого"],
        "list order"
    );
    assert!(!choices.checked.contains(&"нет такого".to_owned()));
}

#[test]
fn own_recipes_are_renamed_and_removed_with_their_check() {
    let mut choices = AiChoices::default();
    put(&mut choices, None, "Лого", "Вставь лого").unwrap();
    let taken = |name: &str| Err(RecipeError::Taken(name.to_owned()));
    assert_eq!(
        put(&mut choices, None, "Субтитры", "x"),
        taken("Субтитры"),
        "a built-in title"
    );
    assert_eq!(put(&mut choices, None, "Лого", "x"), taken("Лого"));
    assert_eq!(put(&mut choices, None, "", "x"), Err(RecipeError::NoName));
    assert_eq!(
        put(&mut choices, None, "Ещё", " "),
        Err(RecipeError::NoPrompt)
    );
    put(&mut choices, Some("Лого"), "Логотип", "Вставь логотип").unwrap();
    assert!(choices.checked.contains(&"Логотип".to_owned()));
    assert_eq!(choices.own.len(), 1);
    choices.edit(AiEdit::Remove("Логотип".to_owned())).unwrap();
    assert!(choices.own.is_empty() && !choices.checked.contains(&"Логотип".to_owned()));
    let missing = choices.edit(AiEdit::Remove("Нет".to_owned()));
    assert_eq!(missing, Err(RecipeError::Missing("Нет".to_owned())));
}

#[test]
fn the_agent_and_its_model_are_remembered() {
    let mut choices = AiChoices::default();
    choices.edit(AiEdit::Agent(Some(Agent::Codex))).unwrap();
    choices.edit(AiEdit::Model(Some("  ".to_owned()))).unwrap();
    assert_eq!(
        (choices.agent, choices.model.clone()),
        (Some(Agent::Codex), None)
    );
    choices
        .edit(AiEdit::Model(Some(" gpt-5 ".to_owned())))
        .unwrap();
    assert_eq!(choices.model.as_deref(), Some("gpt-5"));
    let json = serde_json::to_string(&choices).unwrap();
    assert_eq!(serde_json::from_str::<AiChoices>(&json).unwrap(), choices);
}
