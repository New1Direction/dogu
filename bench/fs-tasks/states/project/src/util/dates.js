export function today() {
  return new Date().toISOString().slice(0, 10);
}
// TODO(ines): time zones
export const EPOCH = 0;
