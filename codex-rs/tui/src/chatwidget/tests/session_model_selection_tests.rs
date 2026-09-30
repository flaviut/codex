//! Exercise session-only model selection through picker key events.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn model_picker_includes_profile_models_after_catalog_refresh() {
    let (mut chat, mut events, _ops) = make_chatwidget_manual(Some("gpt-5.5")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.config
        .model_providers
        .insert("openrouter".into(), chat.config.model_provider.clone());
    let temp = tempdir().expect("tempdir");
    chat.config.config_layer_stack = ConfigLayerStack::default()
        .with_user_config(
            &temp.path().join("config.toml").abs(),
            toml::from_str::<TomlValue>(
                r#"
[profiles.glm53]
model = "z-ai/glm-5.3"
model_provider = "openrouter"
[profiles.glm_duplicate]
model = "z-ai/glm-5.3"
[profiles.unavailable]
model = "missing-model"
model_provider = "missing"
"#,
            )
            .expect("profile config"),
        )
        .expect("valid config");
    chat.open_model_popup();
    let request_id = chat.model_popup_request_id.expect("model fetch");
    let preset = get_available_model(&chat, "gpt-5.5");
    assert!(chat.on_models_loaded(request_id, Ok(vec![preset])));
    chat.refresh_open_model_picker();
    assert_eq!(chat.model_popup_model_ids, vec!["gpt-5.5", "z-ai/glm-5.3"]);
    assert_chatwidget_snapshot!(
        "model_picker_profile_models",
        render_bottom_popup(&chat, /*width*/ 90)
    );
    while events.try_recv().is_ok() {}
    chat.handle_key_event(KeyCode::Down.into());
    chat.handle_key_event(KeyCode::Char('s').into());
    assert_matches!(events.try_recv(), Ok(AppEvent::SelectSessionModel { model, effort })
        if model == "z-ai/glm-5.3" && effort == Some(ReasoningEffortConfig::None));
    assert!(chat.bottom_pane.no_modal_or_popup_active());
}

#[tokio::test]
async fn model_picker_filter_applies_to_catalog_and_profile_models() {
    let (mut chat, _events, _ops) = make_chatwidget_manual(Some("gpt-5.5")).await;
    chat.config.tui_model_picker_filter = Some("^(gpt-5\\.5|z-ai/glm-5\\.3)$".into());
    chat.config
        .model_providers
        .insert("openrouter".into(), chat.config.model_provider.clone());
    let temp = tempdir().expect("tempdir");
    chat.config.config_layer_stack = ConfigLayerStack::default()
        .with_user_config(
            &temp.path().join("config.toml").abs(),
            toml::from_str::<TomlValue>(
                r#"
[profiles.glm53]
model = "z-ai/glm-5.3"
model_provider = "openrouter"
[profiles.other]
model = "other-model"
model_provider = "openrouter"
"#,
            )
            .expect("profile config"),
        )
        .expect("valid config");
    let mut hidden = get_available_model(&chat, "gpt-5.5");
    hidden.id = "hidden-model".into();
    hidden.model = "hidden-model".into();
    hidden.display_name = "Hidden model".into();
    let catalog = vec![get_available_model(&chat, "gpt-5.5"), hidden];
    chat.open_model_popup_with_presets(catalog.clone());
    assert_eq!(chat.model_popup_model_ids, vec!["gpt-5.5", "z-ai/glm-5.3"]);
    assert_chatwidget_snapshot!(
        "model_picker_filtered_models",
        render_bottom_popup(&chat, /*width*/ 90)
    );

    Arc::make_mut(&mut chat.model_catalog).models = catalog;
    chat.open_all_models_popup();
    assert_eq!(chat.model_popup_model_ids, vec!["gpt-5.5", "z-ai/glm-5.3"]);
}

#[tokio::test]
async fn profile_model_picker_preserves_explicit_provider_and_effort() {
    let (mut chat, _events, _ops) = make_chatwidget_manual(Some("gpt-5.5")).await;
    chat.config
        .model_providers
        .insert("openrouter".into(), chat.config.model_provider.clone());
    let temp = tempdir().expect("tempdir");
    chat.config.config_layer_stack = ConfigLayerStack::default()
        .with_user_config(
            &temp.path().join("config.toml").abs(),
            toml::from_str::<TomlValue>(
                r#"
[profiles.router]
model = "gpt-5.5"
model_provider = "openrouter"
model_reasoning_effort = "high"
"#,
            )
            .expect("profile config"),
        )
        .expect("valid config");
    let models = chat.with_profile_models(Vec::new());
    assert_eq!(models.len(), 1);
    assert_eq!(
        (
            models[0].model.as_str(),
            &models[0].default_reasoning_effort
        ),
        ("openrouter::gpt-5.5", &ReasoningEffortConfig::High)
    );
}

#[tokio::test]
async fn inline_model_command_selects_a_provider_model_without_saving() {
    let (mut chat, mut events, _ops) = make_chatwidget_manual(Some("gpt-5.5")).await;
    chat.config
        .model_providers
        .insert("openrouter".into(), chat.config.model_provider.clone());
    for selector in ["z-ai/glm-5.3", "openrouter::z-ai/glm-5.3", "gpt-5.5"] {
        while events.try_recv().is_ok() {}
        chat.bottom_pane
            .set_composer_text(format!("/model {selector}"), Vec::new(), Vec::new());
        chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let selection = std::iter::from_fn(|| events.try_recv().ok())
            .find(|event| matches!(event, AppEvent::SelectSessionModel { .. }));
        assert_matches!(selection, Some(AppEvent::SelectSessionModel { model, effort: None }) if model == selector);
    }
    while events.try_recv().is_ok() {}
    chat.dispatch_command_with_args(SlashCommand::Model, "missing::model".into(), Vec::new());
    assert!(
        std::iter::from_fn(|| events.try_recv().ok())
            .all(|event| !matches!(event, AppEvent::SelectSessionModel { .. }))
    );
}

#[tokio::test]
async fn session_model_selection_accepts_final_choices_without_saving() {
    for picker in [
        "auto",
        "single",
        "default_only",
        "reasoning",
        "max",
        "ultra",
    ] {
        let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.5")).await;
        let effort = match picker {
            "max" => ReasoningEffortConfig::Max,
            "ultra" => ReasoningEffortConfig::Ultra,
            _ => ReasoningEffortConfig::High,
        };
        let mut preset = get_available_model(&chat, "gpt-5.5");
        preset.default_reasoning_effort = effort.clone();
        preset.supported_reasoning_efforts = vec![ReasoningEffortPreset {
            effort: effort.clone(),
            description: "Selected effort".into(),
        }];
        if picker == "auto" {
            preset.model = "codex-auto-test".into();
        }
        let expected_model = preset.model.clone();
        if picker == "default_only" {
            preset.supported_reasoning_efforts.clear();
        }
        match picker {
            "auto" | "single" | "default_only" => chat.open_model_popup_with_presets(vec![preset]),
            "max" | "ultra" => chat.open_advanced_reasoning_popup(preset),
            "reasoning" => {
                chat.set_reasoning_effort(Some(effort.clone()));
                preset
                    .supported_reasoning_efforts
                    .push(ReasoningEffortPreset {
                        effort: ReasoningEffortConfig::Low,
                        description: "Low effort".into(),
                    });
                chat.open_reasoning_popup(preset);
            }
            _ => unreachable!(),
        }
        while rx.try_recv().is_ok() {}
        chat.handle_key_event(KeyEvent::from(KeyCode::Char('s')));
        let selected = rx.try_recv().expect("session-only selection");
        assert_matches!(selected, AppEvent::SelectSessionModel { model, effort: selected_effort }
            if model == expected_model && selected_effort.as_ref() == Some(&effort));
        assert!(chat.bottom_pane.no_modal_or_popup_active(), "{picker}");
        assert!(
            std::iter::from_fn(|| rx.try_recv().ok()).all(|event| matches!(
                event,
                AppEvent::InsertHistoryCell(_) | AppEvent::SettingsSelectionClosed
            )),
            "{picker}"
        );
    }
}

#[tokio::test]
async fn session_model_selection_notifies_the_original_task_after_each_final_astra_choice() {
    for picker in ["model", "reasoning", "advanced"] {
        let (mut chat, mut events, _ops) = make_chatwidget_manual(Some("gpt-5.5")).await;
        let thread_id = ThreadId::new();
        chat.thread_id = Some(thread_id);
        let mut preset = get_available_model(&chat, "gpt-5.5");
        preset.model = "gpt-6-astra".into();
        let effort = if picker == "advanced" {
            ReasoningEffortConfig::Max
        } else {
            ReasoningEffortConfig::High
        };
        preset.default_reasoning_effort = effort.clone();
        preset.supported_reasoning_efforts = vec![ReasoningEffortPreset {
            effort: effort.clone(),
            description: "Selected effort".into(),
        }];
        match picker {
            "model" => chat.open_model_popup_with_presets(vec![preset]),
            "reasoning" => {
                preset
                    .supported_reasoning_efforts
                    .push(ReasoningEffortPreset {
                        effort: ReasoningEffortConfig::Low,
                        description: "Low effort".into(),
                    });
                chat.open_reasoning_popup(preset);
            }
            "advanced" => chat.open_advanced_reasoning_popup(preset),
            _ => unreachable!(),
        }
        while events.try_recv().is_ok() {}
        chat.handle_key_event(KeyCode::Char('s').into());

        assert_matches!(events.try_recv(), Ok(AppEvent::AstraSelectedFromModelPicker {
            thread_id: selected_thread,
            model,
            action: AstraModelPickerAction::SelectSessionModel { effort: selected_effort },
        }) if selected_thread == thread_id && model == "gpt-6-astra" && selected_effort == Some(effort));
        assert!(
            std::iter::from_fn(|| events.try_recv().ok()).all(|event| matches!(
                event,
                AppEvent::InsertHistoryCell(_) | AppEvent::SettingsSelectionClosed
            ))
        );
        assert!(chat.bottom_pane.no_modal_or_popup_active(), "{picker}");
    }
}

#[tokio::test]
async fn session_model_selection_hides_conflicting_shortcut() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.5")).await;
    let mut config = codex_config::types::TuiKeymap::default();
    config.list.accept = Some(codex_config::types::KeybindingsSpec::One(
        codex_config::types::KeybindingSpec("s".into()),
    ));
    let keymap = RuntimeKeymap::from_config(&config).expect("valid list keymap");
    chat.bottom_pane.set_keymap_bindings(&keymap);
    let preset = get_available_model(&chat, "gpt-5.5");
    chat.open_reasoning_popup(preset);
    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(!popup.contains("s session"), "{popup}");
    while rx.try_recv().is_ok() {}
    chat.handle_key_event(KeyEvent::from(KeyCode::Char('s')));
    assert!(
        std::iter::from_fn(|| rx.try_recv().ok())
            .any(|event| matches!(event, AppEvent::PersistModelSelection { .. }))
    );
}
