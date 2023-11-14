use chrono::Datelike;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum Holiday {
  And(Box<Holiday>, Box<Holiday>),
  Date { month: chrono::Month, day: u8, weekend_adjust: bool },
  DayOfMonth(std::collections::BTreeSet<u8>),
  DayOfWeek(std::collections::HashSet<chrono::Weekday>),
  Easter(i8),
  IsoWeek(std::collections::BTreeSet<u8>),
  Month(std::collections::BTreeSet<chrono::Month>),
  Not(Box<Holiday>),
  WeekDay { day: chrono::Weekday, month: chrono::Month, occurrence: u8, ascending: bool },
}

impl Holiday {
  pub fn is_holiday(&self, date: &chrono::NaiveDate) -> bool {
    match self {
      Holiday::And(left, right) => left.is_holiday(date) && right.is_holiday(date),
      Holiday::Date { month, day, weekend_adjust } => match chrono::NaiveDate::from_ymd_opt(date.year(), month.number_from_month(), *day as u32) {
        None => false,
        Some(base) => if *weekend_adjust {
          match date.weekday() {
            chrono::Weekday::Sat => base.pred_opt(),
            chrono::Weekday::Sun => base.succ_opt(),
            _ => Some(base),
          }
        } else {
          Some(base)
        }
        .map(|r| date == &r)
        .unwrap_or(false),
      },
      Holiday::DayOfMonth(days) => days.contains(&(date.day() as u8)),
      Holiday::DayOfWeek(days) => days.contains(&date.weekday()),
      Holiday::Easter(offset) => {
        // Century
        let c = (date.year() / 100) + 1;

        // Shifted
        let mut se = (14 + 11 * (date.year() % 19) - 3 * c / 4 + (5 + 8 * c) / 25) % 30;

        // Adjust
        if (se == 0) || ((se == 1) && (10 < (date.year() % 19))) {
          se += 1;
        }

        // Paschal Moon
        chrono::NaiveDate::from_ymd_opt(date.year(), 4, 19)
          .map(|pm| {
            let p = pm.num_days_from_ce() - se;
            // Easter: local the Sunday after the Paschal Moon
            if *offset < 0 {
              date.checked_add_days(chrono::Days::new(-*offset as u64))
            } else {
              date.checked_sub_days(chrono::Days::new(*offset as u64))
            }
            .map(|d| p + 7 - (p % 7) == d.num_days_from_ce())
            .unwrap_or(false)
          })
          .unwrap_or(false)
      }
      Holiday::IsoWeek(weeks) => date.iso_week().week().try_into().map(|v| weeks.contains(&v)).unwrap_or(false),
      Holiday::Month(months) => chrono::Month::try_from(date.month() as u8).map(|m| months.contains(&m)).unwrap_or(false),
      Holiday::Not(expr) => !expr.is_holiday(date),
      Holiday::WeekDay { day, month, occurrence, ascending } => chrono::NaiveDate::from_ymd_opt(date.year(), month.number_from_month(), 1)
        .map(|base| {
          let offset = chrono::Days::new(
            ((day.number_from_monday() + 7 - base.weekday().number_from_monday()) % 7 + 7 * (*occurrence as u32).checked_sub(1).unwrap_or(0)) as u64,
          );
          if *ascending {
            base.checked_add_days(offset)
          } else {
            base.checked_sub_days(offset)
          }
        })
        .flatten()
        .map(|r| date == &r)
        .unwrap_or(false),
    }
  }
}
