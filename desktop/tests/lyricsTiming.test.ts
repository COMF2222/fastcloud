import assert from 'node:assert/strict'
import { test } from 'node:test'
import { currentLyricLine, parseLrc } from '../src/lyricsTiming.ts'

test('LRC preserves fractions, repeated timestamps, Unicode and instrumental gaps', () => {
  const lines = parseLrc('[ar:Artist]\n[00:02.5][00:05.050]Строка\n[00:01.123]\n[00:03:25]Next')
  assert.deepEqual(lines, [
    { time: 1123, text: '' }, { time: 2500, text: 'Строка' },
    { time: 3250, text: 'Next' }, { time: 5050, text: 'Строка' },
  ])
})

test('highlight changes at the timestamp and follows a backward seek immediately', () => {
  const lines = parseLrc('[00:01.00]First\n[00:02.00]Second\n[00:03.00]Third')
  assert.equal(currentLyricLine(lines, 999), -1)
  assert.equal(currentLyricLine(lines, 1000), 0)
  assert.equal(currentLyricLine(lines, 1999), 0)
  assert.equal(currentLyricLine(lines, 2000), 1)
  assert.equal(currentLyricLine(lines, 3050), 2)
  assert.equal(currentLyricLine(lines, 1050), 0)
  assert.equal(currentLyricLine([], 10_000), -1)
})
