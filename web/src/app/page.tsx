import { Navbar } from "@/components/navbar";
import { Hero } from "@/components/hero";
import { Mockup } from "@/components/mockup";
import { Features } from "@/components/features";
import { HowItWorks } from "@/components/how-it-works";
import { CliSection } from "@/components/cli-section";
import { Cta } from "@/components/cta";
import { Footer } from "@/components/footer";

export default function Home() {
  return (
    <>
      <Navbar />
      <div className="max-w-[960px] mx-auto px-8">
        <Hero />
        <Mockup />
        <Features />
        <HowItWorks />
        <CliSection />
        <Cta />
      </div>
      <Footer />
    </>
  );
}
