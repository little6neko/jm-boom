import { createFileRoute, Outlet, useRouterState } from '@tanstack/react-router'
import { BookmarkIcon, CompassIcon, DownloadIcon, SettingsIcon, ShellIcon } from 'lucide-react'

import {
  FloatingNav,
  resolveFloatingNavActiveId,
  type FloatingNavItem
} from '@/components/floating-nav'

export const Route = createFileRoute('/_app')({
  component: AppRoute
})

const NAV_ITEMS: FloatingNavItem[] = [
  { id: 'bookshelf', icon: ShellIcon, label: '书架', to: '/bookshelf' },
  { id: 'explore', icon: CompassIcon, label: '探索', to: '/explore' },
  { id: 'favorites', icon: BookmarkIcon, label: '收藏', to: '/favorites' },
  { id: 'downloads', icon: DownloadIcon, label: '下载', to: '/downloads' },
  { id: 'settings', icon: SettingsIcon, label: '设置', to: '/settings' }
]

function AppRoute() {
  const pathname = useRouterState({
    select: state => state.location.pathname
  })
  const activeId = resolveFloatingNavActiveId(pathname, NAV_ITEMS)

  return (
    <div className="relative min-h-dvh">
      <FloatingNav items={NAV_ITEMS} activeId={activeId} />
      <Outlet />
    </div>
  )
}
