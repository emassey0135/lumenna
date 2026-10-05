//! What the app offers to choose from that takes more than one listing to find.

use lumenna_surface::Lumenna;

use crate::speech::Clock;

/// A work block a task could be put in.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct BlockChoice {
    /// The occurrence, `<series>@<date>`, as `assign` takes it.
    pub id: String,
    /// Its day, ISO.
    pub date: String,
    /// How it reads in a list: "Tomorrow, 9:00 AM to 11:00 AM, Deep work".
    pub text: String,
}

/// The work blocks of `days` days from `from`, in order, as a chooser lists them — what
/// putting a task in a block offers from the task itself. Which blocks those are is the
/// surface's ([`Lumenna::work_blocks`]); only how each reads is decided here. A failure offers
/// nothing, as an empty week does.
pub fn work_blocks(lumenna: &Lumenna, from: jiff::civil::Date, days: i64, clock: &dyn Clock) -> Vec<BlockChoice> {
    let days = u32::try_from(days).ok();
    lumenna
        .work_blocks(Some(from.to_string()), days)
        .map(|found| found.blocks)
        .unwrap_or_default()
        .into_iter()
        .map(|block| BlockChoice {
            text: format!(
                "{}, {} to {}, {}",
                clock.day(&block.date),
                clock.time(&block.start),
                clock.time(&block.end),
                block.title
            ),
            id: block.id,
            date: block.date,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use lumenna_surface::NewBlock;

    use super::*;
    use crate::speech::testing::TwelveHour;

    fn block(title: &str, at: &str, kind: &str, date: &str, repeat: Option<&str>) -> NewBlock {
        NewBlock {
            title: title.to_owned(),
            at: at.to_owned(),
            minutes: 60,
            date: Some(date.to_owned()),
            kind: kind.to_owned(),
            repeat: repeat.map(str::to_owned),
            ..NewBlock::default()
        }
    }

    #[test]
    fn the_week_offers_its_work_blocks_in_order_and_nothing_else() {
        let directory = tempfile::tempdir().unwrap();
        let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
        lumenna.add_block(block("Deep work", "9am", "work", "2026-10-05", Some("every day"))).unwrap();
        lumenna.add_block(block("Lunch", "12pm", "break", "2026-10-05", Some("every day"))).unwrap();
        lumenna.add_block(block("Review", "4pm", "work", "2026-10-06", None)).unwrap();
        let from: jiff::civil::Date = "2026-10-05".parse().unwrap();
        let choices = work_blocks(&lumenna, from, 2, &TwelveHour);
        let texts: Vec<&str> = choices.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "2026-10-05, 9:00 AM to 10:00 AM, Deep work",
                "2026-10-06, 9:00 AM to 10:00 AM, Deep work",
                "2026-10-06, 4:00 PM to 5:00 PM, Review",
            ]
        );
        assert_eq!(choices[1].date, "2026-10-06");
        assert!(choices[1].id.ends_with("@2026-10-06"), "an occurrence names its day");
    }
}
