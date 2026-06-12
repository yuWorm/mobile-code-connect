import { describe, expect, test } from 'bun:test'

import { buildGithubOAuthStartPath, githubOAuthCallbackUrl } from '../oauth'

describe('github oauth helpers', () => {
  test('builds a frontend callback url from the current origin', () => {
    expect(githubOAuthCallbackUrl('https://console.example.com/admin/users')).toBe(
      'https://console.example.com/login/oauth/github/callback',
    )
  })

  test('builds a frontend callback url with a return target', () => {
    expect(githubOAuthCallbackUrl('https://console.example.com/center/account', '/center/account')).toBe(
      'https://console.example.com/login/oauth/github/callback?redirect=%2Fcenter%2Faccount',
    )
  })

  test('builds the backend start path with an encoded redirect uri', () => {
    expect(
      buildGithubOAuthStartPath('https://console.example.com/login/oauth/github/callback'),
    ).toBe(
      '/auth/oauth/github/start?redirect_uri=https%3A%2F%2Fconsole.example.com%2Flogin%2Foauth%2Fgithub%2Fcallback',
    )
  })
})
