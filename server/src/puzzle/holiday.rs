const SERIALIZATION_LENGTH: u32 = 1;
struct Holiday<C: bdays::HolidayCalendar<chrono::DateTime<chrono::Local>> + Send + Sync + 'static> {
  calendar: std::sync::Arc<C>,
  last: bool,
}

pub struct HolidayAsset<C: bdays::HolidayCalendar<chrono::DateTime<chrono::Local>> + Send + Sync + 'static> {
  pub calendar: std::sync::Arc<C>,
}

pub struct Easter;

impl<C: bdays::HolidayCalendar<chrono::DateTime<chrono::Local>> + Send + Sync> crate::puzzle::PuzzleAsset for HolidayAsset<C> {
  fn create(
    self: Box<Self>,
    time: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> Box<dyn crate::puzzle::PuzzlePiece> {
    Box::new(Holiday { last: self.calendar.is_holiday(time.with_timezone(&chrono::Local)), calendar: self.calendar })
      as Box<dyn crate::puzzle::PuzzlePiece>
  }
  fn load<'a>(
    self: Box<Self>,
    input: &mut crate::puzzle::InputBuffer,
    _: &chrono::DateTime<chrono::Utc>,
    _: &mut std::collections::BTreeMap<String, super::RadioSharedState>,
  ) -> crate::puzzle::DeserializationResult<'a> {
    crate::puzzle::check_length(input, SERIALIZATION_LENGTH)?;
    Ok(Box::new(Holiday { calendar: self.calendar, last: rmp::decode::read_bool(input)? }) as Box<dyn crate::puzzle::PuzzlePiece>)
  }
}

impl<C: bdays::HolidayCalendar<chrono::DateTime<chrono::Local>> + Send + Sync> crate::puzzle::PuzzlePiece for Holiday<C> {
  fn accept<'a>(
    self: &mut Self,
    _: &puzzleverse_core::PuzzleCommand,
    _: &puzzleverse_core::asset::rules::PieceValue,
    _: &chrono::DateTime<chrono::Utc>,
  ) -> crate::puzzle::OutputEvents {
    vec![]
  }
  fn interact(
    self: &mut Self,
    _: &puzzleverse_core::InteractionType,
    _: &str,
    _: Option<u8>,
  ) -> (puzzleverse_core::InteractionResult, crate::puzzle::SimpleOutputEvents) {
    (puzzleverse_core::InteractionResult::Invalid, vec![])
  }

  fn serialize(self: &Self, output: &mut crate::puzzle::OutputBuffer) -> crate::puzzle::SerializationResult {
    rmp::encode::write_array_len(output, SERIALIZATION_LENGTH)?;
    rmp::encode::write_bool(output, self.last).map_err(rmp::encode::ValueWriteError::InvalidDataWrite)?;
    Ok(())
  }
  fn tick(self: &mut Self, time: &chrono::DateTime<chrono::Utc>) -> crate::puzzle::SimpleOutputEvents {
    let current = self.calendar.is_holiday(time.with_timezone(&chrono::Local));
    if current != self.last {
      self.last = current;
      vec![(puzzleverse_core::PuzzleEvent::Changed, puzzleverse_core::asset::rules::PieceValue::Bool(current))]
    } else {
      vec![]
    }
  }
  fn reset(&self) -> crate::puzzle::SimpleOutputEvents {
    vec![]
  }
  fn next(self: &Self) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::Local::now()
      .date_naive()
      .checked_add_days(chrono::Days::new(1))
      .map(|d| d.and_hms_opt(0, 0, 0))
      .flatten()
      .map(|d| d.and_local_timezone(chrono::Utc).latest())
      .flatten()
  }
  fn update_check<'a>(self: &'a Self, _: &std::collections::BTreeSet<u8>) -> Option<super::PuzzleConsequence<'a>> {
    None
  }
  fn walk(
    self: &mut Self,
    _: &crate::PlayerKey,
    _: Option<u8>,
    _: crate::realm::navigation::PlayerNavigationEvent,
  ) -> crate::puzzle::SimpleOutputEvents {
    vec![]
  }
}
impl<T: chrono::Datelike + Copy + PartialOrd> bdays::HolidayCalendar<T> for Easter {
  fn is_holiday(&self, date: T) -> bool {
    date.num_days_from_ce() == bdays::easter::easter_num_days_from_ce(date.year()).unwrap()
  }
}
