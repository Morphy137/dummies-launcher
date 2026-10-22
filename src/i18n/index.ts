import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import en from "./en";
import es from "./es";

function detectSystemLocale(): "en" | "es" {
  const language = window.navigator.language?.toLowerCase() ?? "en";
  return language.startsWith("es") ? "es" : "en";
}

void i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    es: { translation: es },
  },
  lng: detectSystemLocale(),
  fallbackLng: "en",
  supportedLngs: ["en", "es"],
  interpolation: {
    escapeValue: false,
  },
  returnNull: false,
});

export default i18n;
