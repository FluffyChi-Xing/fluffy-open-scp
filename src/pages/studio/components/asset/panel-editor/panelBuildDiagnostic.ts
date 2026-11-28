export function unsupportedPanelItems(message: string): Array<{ view?: 'hover' | 'click'; id: string }> {
  const details = message.match(/\bPANEL_CONTAINER_QUERY_UNSUPPORTED\s*:\s*((?:(?:hover|click):)?[\w-]+(?:,(?:(?:hover|click):)?[\w-]+)*)/)?.[1];
  if (!details) return [];
  return details.split(',').map(item => {
    const [key, id] = item.split(':');
    return id && (key === 'hover' || key === 'click') ? { view: key, id } : { id: key };
  });
}
