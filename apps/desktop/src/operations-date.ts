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
  const milliseconds = new Date(input + (utc ? 'Z' : '')).getTime();
  if (!utc && displayedZone !== systemTimeZone()) return {status: 'zone_changed'};
  return Number.isFinite(milliseconds)
    ? {status: 'ok', value: Math.floor(milliseconds / 1000)}
    : {status: 'invalid'};
}
