import Link from 'next/link';
import type { PageInfo } from '@/lib/pages';
import { breadcrumbLd } from '@/lib/seo';
import { JsonLd } from './JsonLd';
import s from './PageHeader.module.css';

/** The top of every subpage: breadcrumb, the page's one h1, a lead and optional actions. */
export function PageHeader({
  page,
  title,
  lead,
  children,
  extraLd = [],
}: {
  page: PageInfo;
  title: string;
  lead?: React.ReactNode;
  children?: React.ReactNode;
  /** More structured data for this page, merged with the breadcrumb. */
  extraLd?: object[];
}) {
  return (
    <header className={`container ${s.head}`}>
      <JsonLd items={[breadcrumbLd(page), ...extraLd]} />
      <nav aria-label="Breadcrumb" className={s.crumbs}>
        <ol>
          <li>
            <Link href="/">Telex</Link>
          </li>
          <li aria-current="page">{page.label}</li>
        </ol>
      </nav>
      <h1 className="h1">{title}</h1>
      {lead && <p className={s.lead}>{lead}</p>}
      {children && <div className={s.actions}>{children}</div>}
    </header>
  );
}
