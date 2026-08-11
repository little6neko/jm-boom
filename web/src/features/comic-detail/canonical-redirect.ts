export function resolveCanonicalComicRedirect(
  requestedComicId: string,
  canonicalComicId: string | null | undefined
) {
  const targetComicId = canonicalComicId?.trim() ?? ''
  const isPositiveNumericId =
    targetComicId.length > 0 && /^[0-9]+$/.test(targetComicId) && /[1-9]/.test(targetComicId)

  return isPositiveNumericId && targetComicId !== requestedComicId ? targetComicId : null
}
