/** Local editing uses the live system zone; UTC editing never depends on it. */
export function systemTimeZone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone;
}

export function dateInput(seconds: number, timeZone: string): string {
  const parts = new Intl.DateTimeFormat('en-CA', {
    timeZone, year: 'numeric', month: '2-digit', day: '2-digit',
    hour: '2-digit', minute: '2-digit', hourCycle: 'h23',
  }).formatToParts(new Date(seconds * 1000));
  const part = (type: Intl.DateTimeFormatPartTypes) => parts.find(p => p.type === type)!.value;
  return `${part('year')}-${part('month')}-${part('day')}T${part('hour')}:${part('minute')}`;
}

type ParsedDate = {status: 'ok'; value: number} | {status: 'zone_changed'} | {status: 'invalid'};

export function parseDate(input: string, displayedZone: string, utc: boolean): ParsedDate {
  // Check the input's rendered zone, including changes queued before a refresh.
  if (!utc && displayedZone !== systemTimeZone()) return {status: 'zone_changed'};
  // Validate the wall-clock fields in UTC before applying local DST rules.
  // Date otherwise accepts calendar overflow and non-editor date syntax.
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(?::\d{2}(?:\.\d{1,3})?)?$/.test(input)) return {status: 'invalid'};
  const wallTime = new Date(input + 'Z');
  if (!Number.isFinite(wallTime.getTime()) || wallTime.toISOString().slice(0, 16) !== input.slice(0, 16)) return {status: 'invalid'};
  const milliseconds = new Date(input + (utc ? 'Z' : '')).getTime();
  if (!utc && displayedZone !== systemTimeZone()) return {status: 'zone_changed'};
  return Number.isFinite(milliseconds)
    ? {status: 'ok', value: Math.floor(milliseconds / 1000)}
    : {status: 'invalid'};
}
