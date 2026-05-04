import { Code2, File, FileText, Image as ImageIcon, Link2, Mail, Palette, Type } from "lucide-react";
import React from "react";

export function getTypeLabel(type: string): string {
  switch (type) {
    case "text": return "Text";
    case "url": return "Link";
    case "code": return "Code Snippet";
    case "image": return "Image";
    case "color": return "Color Code";
    case "email": return "Email Address";
    case "file": return "File";
    default: return "Text";
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
