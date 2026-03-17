import { PostHog } from 'posthog-node';
import { randomUUID, createHash } from 'crypto';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';

const POSTHOG_API_KEY = 'phc_r5UQ9LyZzHcQnpSt0d6xP62q0RMFSSals2HgEQGfRdT';
const POSTHOG_HOST = 'https://eu.i.posthog.com';

const CONFIG_DIR = path.join(os.homedir(), '.rippletide');
const CONFIG_FILE = path.join(CONFIG_DIR, 'config.json');
const ANALYTICS_ENABLED_KEY = 'analytics_enabled';
const DISTINCT_ID_KEY = 'distinct_id';

interface Config {
  [ANALYTICS_ENABLED_KEY]?: boolean;
  [DISTINCT_ID_KEY]?: string;
  first_seen?: string;
  total_launches?: number;
}

class Analytics {
  private client: PostHog | null = null;
  private distinctId: string;
  private enabled: boolean = true;
  private pendingFlushes: Promise<any>[] = [];

  constructor() {
    this.distinctId = this.getOrCreateDistinctId();
    this.enabled = this.isAnalyticsEnabled();
    
    if (this.enabled) {
      this.initClient();
      this.identifyUser();
    }
  }

  private initClient(): void {
    if (!this.client) {
      this.client = new PostHog(POSTHOG_API_KEY, {
        host: POSTHOG_HOST,
        flushAt: 1,
        flushInterval: 0,
        disableGeoip: false,
      });
      
      if (process.env.DEBUG_ANALYTICS) {
        console.log('[Analytics] Client initialized with ID:', this.distinctId);
      }
      
      const cleanup = () => {
        if (this.client) {
          this.client.shutdown();
        }
      };
      
      process.on('beforeExit', cleanup);
      process.on('SIGINT', cleanup);
      process.on('SIGTERM', cleanup);
    }
  }

  private getConfig(): Config {
    try {
      if (!fs.existsSync(CONFIG_FILE)) {
        return {};
      }
      const content = fs.readFileSync(CONFIG_FILE, 'utf-8');
      return JSON.parse(content);
    } catch {
      return {};
    }
  }

  private saveConfig(config: Config): void {
    try {
      if (!fs.existsSync(CONFIG_DIR)) {
        fs.mkdirSync(CONFIG_DIR, { recursive: true });
      }
      fs.writeFileSync(CONFIG_FILE, JSON.stringify(config, null, 2));
    } catch (error) {
      console.error('Failed to save config:', error);
    }
  }

  private getOrCreateDistinctId(): string {
    const config = this.getConfig();
    
    if (config[DISTINCT_ID_KEY]) {
      return config[DISTINCT_ID_KEY];
    }

    const newId = randomUUID();
    this.saveConfig({ ...config, [DISTINCT_ID_KEY]: newId });
    return newId;
  }

  private isAnalyticsEnabled(): boolean {
    const envDisabled = process.env.RIPPLETIDE_NO_ANALYTICS === '1' || 
                       process.env.RIPPLETIDE_NO_ANALYTICS === 'true';
    
    if (envDisabled) {
      return false;
    }

    const config = this.getConfig();
    return config[ANALYTICS_ENABLED_KEY] !== false;
  }

  public setAnalyticsEnabled(enabled: boolean): void {
    const config = this.getConfig();
    this.saveConfig({ ...config, [ANALYTICS_ENABLED_KEY]: enabled });
    this.enabled = enabled;
    
    if (!enabled && this.client) {
      this.client.shutdown();
      this.client = null;
    } else if (enabled && !this.client) {
      this.client = new PostHog(POSTHOG_API_KEY, {
        host: POSTHOG_HOST,
        flushAt: 1,
        flushInterval: 0,
      });
    }
  }

  public track(event: string, properties?: Record<string, any>): void {
    if (!this.client || !this.enabled) {
      if (process.env.DEBUG_ANALYTICS) {
        console.log('[Analytics] Skipped:', event, '- Client:', !!this.client, 'Enabled:', this.enabled);
      }
      return;
    }

    try {
      if (process.env.DEBUG_ANALYTICS) {
        console.log('[Analytics] Tracking:', event, properties);
      }
      
      // Capture the event
      this.client.capture({
        distinctId: this.distinctId,
        event,
        properties: {
          ...properties,
          cli_version: process.env.npm_package_version || 'unknown',
          node_version: process.version,
          platform: os.platform(),
          arch: os.arch(),
          timestamp: new Date().toISOString(),
        },
      });
      
      // Force synchronous flush for CLI
      const flushPromise = this.client.flush();
      
      if (process.env.DEBUG_ANALYTICS) {
        flushPromise.then(() => {
          console.log('[Analytics] Event flushed:', event);
        }).catch((err) => {
          console.error('[Analytics] Flush error:', err);
        });
      }
    } catch (error) {
      if (process.env.DEBUG_ANALYTICS) {
        console.error('[Analytics] Error:', error);
      }
    }
  }

  private identifyUser(): void {
    if (!this.client || !this.enabled) return;

    try {
      // Get machine info for better user identification
      const config = this.getConfig();
      const firstSeen = config.first_seen || new Date().toISOString();
      const totalLaunches = (config.total_launches || 0) + 1;
      
      this.saveConfig({ 
        ...config, 
        first_seen: firstSeen,
        total_launches: totalLaunches
      });

      this.client.identify({
        distinctId: this.distinctId,
        properties: {
          first_seen: firstSeen,
          total_launches: totalLaunches,
          platform: os.platform(),
          arch: os.arch(),
          node_version: process.version,
          cli_version: process.env.npm_package_version || 'unknown',
          hostname_hash: createHash('sha256').update(os.hostname()).digest('hex').substring(0, 8),
        },
      });
      
      this.client.capture({
        distinctId: this.distinctId,
        event: '$set',
        properties: {
          $set: {
            platform: os.platform(),
            arch: os.arch(),
            total_launches: totalLaunches,
          }
        }
      });
      
      if (process.env.DEBUG_ANALYTICS) {
        console.log('[Analytics] User identified:', this.distinctId, 'Launch #', totalLaunches);
      }
    } catch (error) {
      if (process.env.DEBUG_ANALYTICS) {
        console.error('[Analytics] Identify error:', error);
      }
    }
  }

  public identify(properties?: Record<string, any>): void {
    if (!this.client || !this.enabled) return;

    try {
      this.client.identify({
        distinctId: this.distinctId,
        properties,
      });
    } catch (error) {
    }
  }

  public async shutdown(): Promise<void> {
    if (this.client) {
      if (process.env.DEBUG_ANALYTICS) {
        console.log('[Analytics] Shutting down...');
      }
      await this.client.shutdown();
      if (process.env.DEBUG_ANALYTICS) {
        console.log('[Analytics] Shutdown complete');
      }
    }
  }
}

export const analytics = new Analytics();