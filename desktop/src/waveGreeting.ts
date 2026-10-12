export type Daypart = 'morning' | 'day' | 'evening' | 'night'

const greetings: Record<Daypart, readonly (readonly [string, string])[]> = {
  morning: [['Доброе утро', 'Good morning'], ['Хорошего утра', 'Have a lovely morning'], ['Утро в твоём ритме', 'Start the morning your way'], ['Музыка для твоего утра', 'Music for your morning']],
  day: [['Добрый день', 'Good afternoon'], ['Хорошего дня', 'Have a lovely day'], ['День в твоём ритме', 'Make the day yours'], ['Пусть день звучит', 'Give your day a soundtrack']],
  evening: [['Добрый вечер', 'Good evening'], ['Хорошего вечера', 'Enjoy your evening'], ['Вечер в твоём ритме', 'Your evening, your rhythm'], ['Музыка для твоего вечера', 'Music for your evening']],
  night: [['Доброй ночи', 'Good night'], ['Тихой ночи', 'Have a peaceful night'], ['Ночь в твоём ритме', 'Your night, your rhythm'], ['Музыка для этой ночи', 'Music for tonight']],
}

export function daypart(hour: number): Daypart {
  return hour >= 5 && hour < 12 ? 'morning' : hour >= 12 && hour < 17 ? 'day' : hour >= 17 && hour < 23 ? 'evening' : 'night'
}

export function waveGreeting(hour: number, english: boolean, variation: number) {
  const options = greetings[daypart(hour)]
  const index = Number.isFinite(variation) ? Math.abs(Math.trunc(variation)) % options.length : 0
  return options[index][english ? 1 : 0]
}
