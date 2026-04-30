import React, {useEffect, useState} from 'react';
import {useThemeConfig} from '@docusaurus/theme-common';
import FooterLinks from '@theme/Footer/Links';
import FooterLogo from '@theme/Footer/Logo';
import FooterCopyright from '@theme/Footer/Copyright';
import FooterLayout from '@theme/Footer/Layout';

type DocsColorProfile = 'helios-orange' | 'fjord' | 'neon' | 'crimson';

interface ThemeProfile {
  id: DocsColorProfile;
  label: string;
}

const STORAGE_KEY = 'alchemist-docs-color-profile';

const THEME_PROFILES: ThemeProfile[] = [
  {id: 'helios-orange', label: 'Helios Orange'},
  {id: 'fjord', label: 'Fjord'},
  {id: 'neon', label: 'Neon'},
  {id: 'crimson', label: 'Crimson'},
];

function isProfile(value: string | null): value is DocsColorProfile {
  return THEME_PROFILES.some((profile) => profile.id === value);
}

function readProfile(): DocsColorProfile {
  if (typeof document === 'undefined') {
    return 'helios-orange';
  }

  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (isProfile(stored)) {
      return stored;
    }
  } catch {
    // Keep theme selection functional when storage is blocked.
  }

  const documentProfile = document.documentElement.getAttribute('data-color-profile');
  return isProfile(documentProfile) ? documentProfile : 'helios-orange';
}

function applyProfile(profile: DocsColorProfile) {
  if (typeof document === 'undefined') {
    return;
  }

  document.documentElement.setAttribute('data-color-profile', profile);

  try {
    window.localStorage.setItem(STORAGE_KEY, profile);
  } catch {
    // Theme selection should still update the current page without persistence.
  }
}

function DocsThemeSelector() {
  const [activeProfile, setActiveProfile] = useState<DocsColorProfile>('helios-orange');

  useEffect(() => {
    const profile = readProfile();
    setActiveProfile(profile);
    applyProfile(profile);
  }, []);

  const selectProfile = (profile: DocsColorProfile) => {
    setActiveProfile(profile);
    applyProfile(profile);
  };

  return (
    <div className="docs-footer-theme-picker" aria-label="Docs color profile">
      <span className="docs-footer-theme-picker__label">Theme</span>
      <div className="docs-footer-theme-picker__options" role="radiogroup" aria-label="Docs color profile">
        {THEME_PROFILES.map((profile) => (
          <button
            key={profile.id}
            type="button"
            className="docs-footer-theme-picker__option"
            data-active={activeProfile === profile.id ? 'true' : 'false'}
            role="radio"
            aria-checked={activeProfile === profile.id}
            onClick={() => selectProfile(profile.id)}>
            {profile.label}
          </button>
        ))}
      </div>
    </div>
  );
}

function Footer() {
  const {footer} = useThemeConfig();
  if (!footer) {
    return null;
  }

  const {copyright, links, logo, style} = footer;

  return (
    <FooterLayout
      style={style}
      links={links && links.length > 0 && <FooterLinks links={links} />}
      logo={logo && <FooterLogo logo={logo} />}
      copyright={
        <>
          {copyright && <FooterCopyright copyright={copyright} />}
          <DocsThemeSelector />
        </>
      }
    />
  );
}

export default React.memo(Footer);
