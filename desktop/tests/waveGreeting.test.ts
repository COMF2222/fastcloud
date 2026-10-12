import { test } from 'node:test'
import assert from 'node:assert/strict'
import { daypart, waveGreeting } from '../src/waveGreeting.ts'

test('greetings follow local morning, day, evening and night boundaries', () => {
  for (const [hour, part] of [[0,'night'],[4,'night'],[5,'morning'],[11,'morning'],[12,'day'],[16,'day'],[17,'evening'],[22,'evening'],[23,'night']] as const) assert.equal(daypart(hour), part)
  assert.equal(waveGreeting(8, false, 0), 'Доброе утро')
  assert.equal(waveGreeting(23, true, 0), 'Good night')
})

test('each period has multiple translated variants and deterministic render selection', () => {
  for (const hour of [8,14,19,1]) for (const english of [true,false]) {
    const variants = Array.from({ length:4 }, (_, index) => waveGreeting(hour, english, index))
    assert.equal(new Set(variants).size, 4)
    assert.equal(waveGreeting(hour, english, 4), variants[0])
    assert.equal(waveGreeting(hour, english, Number.NaN), variants[0])
  }
})
