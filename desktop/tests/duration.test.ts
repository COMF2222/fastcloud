import { test } from 'node:test'
import assert from 'node:assert/strict'
import { duration } from '../src/types.ts'

test('track and player timestamps display hours for long recordings', () => {
  assert.equal(duration(0), '0:00')
  assert.equal(duration(61_999), '1:01')
  assert.equal(duration(3_599_999), '59:59')
  assert.equal(duration(3_600_000), '1:00:00')
  assert.equal(duration(140 * 60_000), '2:20:00')
  assert.equal(duration(140 * 60_000 + 9000), '2:20:09')
  assert.equal(duration(25 * 3_600_000), '25:00:00')
})

test('missing and invalid track times remain safe', () => {
  for (const value of [undefined, null, NaN, Infinity, -Infinity, -1]) {
    assert.equal(duration(value), '0:00')
  }
})
