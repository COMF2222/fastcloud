export function playbackError(error: string, english: boolean): { message: string; action: 'retry' | 'account' | 'none' } {
  const text = error.toLowerCase()
  const t = (ru: string, en: string) => english ? en : ru
  if (/expired|unauthorized|401|sign in again/.test(text)) return { message: t('Сессия истекла. Войди снова в настройках аккаунта.', 'Your session expired. Sign in again in Account settings.'), action: 'account' }
  if (/429|limit was reached|rate limit/.test(text)) return { message: t('SoundCloud ограничил запросы. Попробуй немного позже.', 'SoundCloud limited requests. Try again later.'), action: 'none' }
  if (/403|404|no stream|blocked|no longer available|does not allow/.test(text)) return { message: t('SoundCloud не предоставляет этот трек для твоего аккаунта.', 'SoundCloud does not make this track available to your account.'), action: 'none' }
  if (/probe|decode|audio|stream/.test(text)) return { message: t('Не удалось воспроизвести аудио. Можно повторить с текущей позиции.', 'Audio playback failed. Retry from the current position.'), action: 'retry' }
  if (/connect|network|timeout|temporarily|could not be read/.test(text)) return { message: t('Соединение прервалось. Проверь сеть и повтори.', 'Connection interrupted. Check your network and retry.'), action: 'retry' }
  return { message: t('Не удалось выполнить действие. Попробуй ещё раз.', 'The action failed. Please try again.'), action: 'retry' }
}
