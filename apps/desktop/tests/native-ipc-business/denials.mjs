/** Classify native responses without treating an arbitrary failure as a denial. */
export function verifyDenial(command, error, registered = false) {
  const message = String(error);
  const kind = /not allowed|not permitted/i.test(message) ? 'permission'
    : /not found/i.test(message) ? 'missing' : 'other';
  if (kind === 'other' || (registered && kind !== 'permission')) {
    throw new Error(`unexpected ${command}: ${message}`);
  }
  return { command, kind };
}
