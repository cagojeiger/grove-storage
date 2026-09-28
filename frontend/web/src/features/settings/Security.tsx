import { ArrowLeft } from "lucide-react";
import { PasswordChange } from "./PasswordChange";

export function Security() {
  return <main className="overview settings">
    <a className="back-link" href="#settings"><ArrowLeft size={16} />My account</a>
    <div className="page-heading">
      <div><p className="eyebrow">MY ACCOUNT</p><h1>Security</h1></div>
    </div>
    <PasswordChange />
  </main>;
}
