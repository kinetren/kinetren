//! Кэш burst'ов. Единственная правда о «прогретости» (инвариант №4):
//! coverage для UI — это запрос к самому кэшу, никакой отдельной бухгалтерии.
//!
//! Ключ: (source_id, kind{Audio | Video{quality}}, idx). Quality — часть ключа:
//! записи разных качеств не взаимозаменяемы (в старом коде это было источником
//! ложного «тепла» — дефект g в docs/porting-notes.md).
//!
//! Реализация — этап M2: LRU с раздельными байтовыми бюджетами аудио/видео,
//! interval-set для coverage(), pin для головы loop-региона, epoch-счётчик
//! для дешёвого «полоски не менялись — не перерисовывай».

use kt_media::Quality;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BurstKind {
    Audio,
    Video { quality: Option<Quality> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BurstKey {
    pub source: SourceId,
    pub kind: BurstKind,
    pub idx: i64,
}
