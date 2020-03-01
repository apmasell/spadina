struct Holiday<C: bdays::HolidayCalendar<chrono::DateTime<chrono::Local>> + Send + Sync + 'static> {
  calendar: std::sync::Arc<C>,
  last: bool,
}

pub struct HolidayAsset<C: bdays::HolidayCalendar<chrono::DateTime<chrono::Local>> + Send + Sync + 'static> {
  pub calendar: std::sync::Arc<C>,
}

pub struct Easter;

impl<C: bdays::HolidayCalendar<chrono::DateTime<chrono::Local>> + Send + Sync> crate::realm::puzzle::PuzzleAsset for HolidayAsset<C> {
  fn create(
    self: Box<Self>,
    time: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> Box<dyn crate::realm::puzzle::PuzzlePiece> {
    Box::new(Holiday { last: self.calendar.is_holiday(time.with_timezone(&chrono::Local)), calendar: self.calendar })
      as Box<dyn crate::realm::puzzle::PuzzlePiece>
  }
  fn load<'a>(
    self: Box<Self>,
    input: serde_json::Value,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<crate::shstr::ShStr, super::RadioSharedState>,
  ) -> crate::realm::puzzle::DeserializationResult<'a> {
    Ok(Box::new(Holiday { calendar: self.calendar, last: serde_json::from_value(input)? }) as Box<dyn crate::realm::puzzle::PuzzlePiece>)
  }
}

impl<C: bdays::HolidayCalendar<chrono::DateTime<chrono::Local>> + Send + Sync> crate::realm::puzzle::PuzzlePiece for Holiday<C> {
  fn accept<'a>(
    self: &mut Self,
    _: &spadina_core::puzzle::PuzzleCommand,
    _: &spadina_core::asset::rules::PieceValue<crate::shstr::ShStr>,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::realm::puzzle::OutputEvents {
    vec![]
  }
  fn interact(
    self: &mut Self,
    _: &spadina_core::realm::InteractionType<crate::shstr::ShStr>,
    _: Option<u8>,
  ) -> (spadina_core::realm::InteractionResult, crate::realm::puzzle::SimpleOutputEvents) {
    (spadina_core::realm::InteractionResult::Invalid, vec![])
  }

  fn serialize(self: &Self) -> crate::realm::puzzle::SerializationResult {
    serde_json::to_value(self.last)
  }
  fn tick(self: &mut Self, time: &chrono::DateTime<chrono::Utc>) -> crate::realm::puzzle::SimpleOutputEvents {
    let current = self.calendar.is_holiday(time.with_timezone(&chrono::Local));
    if current != self.last {
      self.last = current;
      vec![(spadina_core::puzzle::PuzzleEvent::Changed, spadina_core::asset::rules::PieceValue::Bool(current))]
    } else {
      vec![]
    }
  }
  fn reset(&self) -> crate::realm::puzzle::SimpleOutputEvents {
    vec![]
  }
  fn next(self: &Self) -> Option<std::time::Duration> {
    chrono::Local::now()
      .date_naive()
      .checked_add_days(chrono::Days::new(1))
      .map(|d| d.and_hms_opt(0, 0, 0))
      .flatten()
      .map(|d| d.and_local_timezone(chrono::Utc).latest())
      .flatten()
      .map(|d| (d - chrono::Utc::now()).to_std().ok())
      .flatten()
  }
  fn update_check<'a>(self: &'a Self, _: &std::collections::BTreeSet<u8>) -> Option<super::PuzzleConsequence<'a>> {
    None
  }
  fn walk(
    self: &mut Self,
    _: &crate::realm::puzzle::PlayerKey,
    _: Option<u8>,
    _: crate::realm::navigation::PlayerNavigationEvent,
  ) -> crate::realm::puzzle::SimpleOutputEvents {
    vec![]
  }
}
impl<T: chrono::Datelike + Copy + PartialOrd> bdays::HolidayCalendar<T> for Easter {
  fn is_holiday(&self, date: T) -> bool {
    date.num_days_from_ce() == bdays::easter::easter_num_days_from_ce(date.year()).unwrap()
  }
}
