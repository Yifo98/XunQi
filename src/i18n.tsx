import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export type AppLanguage = "zh" | "en";

type I18nContextValue = {
  language: AppLanguage;
  setLanguage: (language: AppLanguage) => void;
  text: (chinese: string, english: string) => string;
};

const STORAGE_KEY = "xunqi.interface-language";
const I18nContext = createContext<I18nContextValue | null>(null);

export function LanguageProvider({ children }: { children: ReactNode }) {
  const [language, setLanguage] = useState<AppLanguage>(() => {
    try {
      return window.localStorage.getItem(STORAGE_KEY) === "en" ? "en" : "zh";
    } catch {
      return "zh";
    }
  });

  useEffect(() => {
    document.documentElement.lang = language === "en" ? "en" : "zh-CN";
    try {
      window.localStorage.setItem(STORAGE_KEY, language);
    } catch {
      // Language switching still works when storage is unavailable.
    }
  }, [language]);

  const text = useCallback(
    (chinese: string, english: string) => (language === "en" ? english : chinese),
    [language],
  );
  const value = useMemo(() => ({ language, setLanguage, text }), [language, text]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

// eslint-disable-next-line react-refresh/only-export-components -- the provider and hook are one public i18n boundary
export function useI18n() {
  const value = useContext(I18nContext);
  if (!value) throw new Error("useI18n must be used inside LanguageProvider");
  return value;
}
