//! Тест 422 (Р.49 Б11): новая редакция инварианта 5 — сторож формулировки.

/// Инвариант 5 переписан: «все клиенты ходят через очередь MediaWorker» было
/// неправдой — своих короткоживущих источников в дереве четыре. Тест сторожит
/// ТЕКСТ правила и его условия: если формулировку вернут к прежней, он
/// покраснеет, а не промолчит.
///
/// Родня «catalog keys nobody uses»: сторожится утверждение, а не код.
#[test]
fn t422_invariant_5_names_the_offline_path() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let md = std::fs::read_to_string(root.join("CLAUDE.md")).expect("нет CLAUDE.md");
    let i = md.find("5. **Экземпляром декодера").expect(
        "инвариант 5 переписан обратно: «все клиенты ходят через очередь» было неправдой — \
         офлайн-путь открывает свои источники",
    );
    let rule = &md[i..md[i..].find("\n6. ").map(|n| i + n).unwrap_or(md.len())];
    for must in [
        "Живое воспроизведение",
        "Офлайн-путь",
        "не делится",
        "темпа не обещает",
        "конечна и отменяема",
    ] {
        assert!(
            rule.contains(must),
            "в инварианте 5 нет условия «{must}» — офлайн-путь снова описан неточно"
        );
    }

    // и сами условия обязаны быть выполнимы: каркас конечной отменяемой
    // задачи существует, а служба индекса умирает вместе с владельцем
    let bg = std::fs::read_to_string(root.join("src/apps/kt-app/src/bg.rs")).expect("нет bg.rs");
    assert!(bg.contains("impl<P, R> Drop for BgJob"), "у задачи нет гашения по Drop");
    let util =
        std::fs::read_to_string(root.join("src/apps/kt-app/src/util.rs")).expect("нет util.rs");
    assert!(util.contains("impl Drop for IndexService"), "служба индекса не умирает с владельцем");
}
