/**
 * Turns the days a person picks into the instants the search API takes.
 *
 * A date input yields `YYYY-MM-DD` in the viewer's own calendar. `after` is
 * inclusive and `before` exclusive, so a range "from the 1st to the 30th"
 * becomes after = local midnight starting the 1st, before = local midnight
 * starting the 31st, and the whole of the 30th is included.
 */
export function startOfLocalDay(day: string, addDays = 0): string | undefined {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day);
  if (!match) return undefined;
  const [, y, m, d] = match.map(Number);
  return new Date(y, m - 1, d + addDays).toISOString();
}

export function searchWindow(fromDay: string, toDay: string): { after?: string; before?: string } {
  return {
    after: fromDay ? startOfLocalDay(fromDay) : undefined,
    before: toDay ? startOfLocalDay(toDay, 1) : undefined,
  };
}
