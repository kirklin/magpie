import type { Locale } from "../i18n";
import { Code2, File, FileText, Image as ImageIcon, Link2, Mail, Palette, Type } from "lucide-react";
import React from "react";
import { t } from "../i18n";

export function getTypeLabel(type: string, locale: Locale = "zh"): string {
  switch (type) {
    case "text": return t(locale, "type.text");
    case "url": return t(locale, "type.url");
    case "code": return t(locale, "type.code");
    case "image": return t(locale, "type.image");
    case "color": return t(locale, "type.color");
    case "email": return t(locale, "type.email");
    case "file": return t(locale, "type.file");
    default: return t(locale, "type.text");
  }
}

export function getTypeIcon(type: string): React.ReactNode {
  const props = { size: 14, strokeWidth: 2 };
  switch (type) {
    case "text": return React.createElement(FileText, props);
    case "url": return React.createElement(Link2, props);
    case "code": return React.createElement(Code2, props);
    case "image": return React.createElement(ImageIcon, props);
    case "color": return React.createElement(Palette, props);
    case "email": return React.createElement(Mail, props);
    case "file": return React.createElement(File, props);
    default: return React.createElement(Type, props);
  }
}
