import assert from 'node:assert/strict';
import {test} from 'node:test';
import {dateInput, parseDate, systemTimeZone} from '../src/operations-date.ts';

const originalZone = process.env.TZ;
function inZone(zone, run) {
  process.env.TZ = zone;
  try { run(); } finally {
    if (originalZone === undefined) delete process.env.TZ;
    else process.env.TZ = originalZone;
  }
}
const instant = Date.parse('2026-10-22T11:47:00Z') / 1000;
test('Taipei and Los Angeles labels match date conversion and parse', () => {
  for (const [zone, expected] of [['Asia/Taipei','2026-10-22T19:47'], ['America/Los_Angeles','2026-10-22T04:47']]) {
    inZone(zone, () => {
      assert.equal(systemTimeZone(), zone);
      assert.equal(dateInput(instant, zone), expected);
      assert.deepEqual(parseDate(expected, zone, false), {status:'ok', value:instant});
    });
  }
});
test('a running page retains consistent old-zone display but cannot write after TZ changes', () => {
  inZone('Asia/Taipei', () => {
    const displayedZone = systemTimeZone();
    const input = dateInput(instant, displayedZone);
    process.env.TZ = 'America/Los_Angeles';
    assert.equal(dateInput(instant, displayedZone), input);
    assert.deepEqual(parseDate(input, displayedZone, false), {status:'zone_changed'});
    const refreshedZone = systemTimeZone();
    assert.equal(refreshedZone, 'America/Los_Angeles');
    assert.deepEqual(parseDate(dateInput(instant, refreshedZone), refreshedZone, false), {status:'ok', value:instant});
    // A queued change from a replaced old input must still fail.
    assert.deepEqual(parseDate(input, displayedZone, false), {status:'zone_changed'});
  });
});

test('Los Angeles to Taipei changes reject old inputs before any poll', () => {
  inZone('America/Los_Angeles', () => {
    const displayedZone = systemTimeZone();
    const input = dateInput(instant, displayedZone);
    process.env.TZ = 'Asia/Taipei';
    assert.equal(dateInput(instant, displayedZone), input);
    assert.deepEqual(parseDate(input, displayedZone, false), {status:'zone_changed'});
    assert.deepEqual(parseDate('2026-10-22T19:47', systemTimeZone(), false), {status:'ok', value:instant});
  });
});
test('UTC editing survives a running process timezone change', () => {
  inZone('Asia/Taipei', () => {
    const input = dateInput(instant, 'UTC');
    process.env.TZ = 'America/Los_Angeles';
    assert.equal(input, '2026-10-22T11:47');
    assert.deepEqual(parseDate(input, 'UTC', true), {status:'ok', value:instant});
  });
});
test('Los Angeles DST offsets use the timestamp rather than the current offset', () => {
  inZone('America/Los_Angeles', () => {
    for (const [utc, local] of [
      ['2026-03-08T09:30:00Z','2026-03-08T01:30'],
      ['2026-03-08T10:30:00Z','2026-03-08T03:30'],
      ['2026-11-01T08:30:00Z','2026-11-01T01:30'],
      ['2026-11-01T09:30:00Z','2026-11-01T01:30'],
      ['2026-11-01T10:30:00Z','2026-11-01T02:30'],
    ]) assert.equal(dateInput(Date.parse(utc)/1000, systemTimeZone()), local);
    // Preserve JavaScript's compatible gap normalization (02:30 becomes 03:30).
    assert.deepEqual(parseDate('2026-03-08T02:30', systemTimeZone(), false), {status:'ok', value:Date.parse('2026-03-08T10:30:00Z')/1000});
    // Match JavaScript's compatible disambiguation: first occurrence in a fold.
    assert.deepEqual(parseDate('2026-11-01T01:30', systemTimeZone(), false), {status:'ok', value:Date.parse('2026-11-01T08:30:00Z')/1000});
  });
});
test('empty or invalid input cannot become epoch zero', () => {
  inZone('Asia/Taipei', () => {
    for (const input of ['', 'bad']) {
      assert.deepEqual(parseDate(input, systemTimeZone(), false), {status:'invalid'});
    }
  });
});
