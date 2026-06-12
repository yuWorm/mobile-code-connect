export function githubOAuthCallbackUrl(currentUrl: string, redirectPath?: string) {
  const url = new URL('/login/oauth/github/callback', currentUrl)
  if (redirectPath) {
    url.searchParams.set('redirect', redirectPath)
  }
  return url.toString()
}

export function buildGithubOAuthStartPath(redirectUri: string) {
  const params = new URLSearchParams({ redirect_uri: redirectUri })
  return `/auth/oauth/github/start?${params.toString()}`
}
