use term::Viewable;

use crate::term::{self, View};

struct Instance {
    iterations: usize,
    warmups: usize,
    time_total: f64,
    time_last: f64,
    warmup_count: usize,
    count: usize,
}

impl Instance {
    fn time_total_clamp(&self, min_count: usize) -> f64 {
        if self.count > min_count {
            self.time_total - self.time_last
        } else {
            self.time_total
        }
    }

    fn avg(&self, min_count: usize) -> Option<f64> {
        if min_count > 0 {
            Some(self.time_total_clamp(min_count) / (min_count as f64))
        } else {
            None
        }
    }
}

fn print_values(values: &[f64], view: &mut View) {
    let width = 32;
    let pad = if view.col() < width {
        width - view.col()
    } else {
        0
    };
    format!("{:1$}", "", pad).view(view);

    let first = *values.first().unwrap();

    for (i, avg) in values.iter().enumerate() {
        term::color(100, 162, 217).view(view);
        format!("{:>8.04}s", avg).view(view);
        term::default_color().view(view);

        if i > 0 {
            let change = 100.0 * ((avg / first) - 1.0);

            if change > 0.5 {
                term::color(219, 126, 94).view(view);
            } else if change < -0.5 {
                term::color(143, 209, 98).view(view);
            }

            format!(" {:+6.02}%", change).view(view);
            term::default_color().view(view);
        }

        if i != values.len() - 1 {
            " ".view(view);
        }
    }
}

struct ConfigInstances {
    config: super::Config,
    builds: Vec<Instance>,
    completed: bool,
    started: bool,
}

impl ConfigInstances {
    fn min_count(&self) -> usize {
        self.builds
            .iter()
            .map(|instance| instance.count)
            .min()
            .unwrap_or(0)
    }

    fn avgs(&self, view: &mut View) {
        let avgs: Option<Vec<f64>> = self
            .builds
            .iter()
            .map(|instance| instance.avg(instance.count))
            .collect();
        avgs.map(|avgs| print_values(&avgs, view));
    }
}

pub struct Display {
    view: View,
    configs: Vec<ConfigInstances>,
    new_line_first: bool,
}

impl Display {
    pub(crate) fn new(
        configs: &Vec<super::ConfigInstances>,
        iterations: usize,
        warmups: usize,
    ) -> Self {
        Display {
            new_line_first: true,
            view: View::new(),
            configs: configs
                .iter()
                .map(|config| ConfigInstances {
                    completed: false,
                    started: false,
                    config: config.config.clone(),
                    builds: config
                        .builds
                        .iter()
                        .map(|_| Instance {
                            iterations,
                            warmups,
                            time_total: 0.0,
                            time_last: 0.0,
                            count: 0,
                            warmup_count: 0,
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    pub fn start_config(&mut self, config_index: usize) {
        self.configs[config_index].started = true;
        self.refresh();
    }

    pub fn report_warmup(&mut self, config_index: usize, build_index: usize) {
        let config = &mut self.configs[config_index];
        let mut instance = &mut config.builds[build_index];
        instance.warmup_count += 1;
        self.refresh();
    }

    pub fn report(&mut self, config_index: usize, build_index: usize, result: f64) {
        let mut config = &mut self.configs[config_index];
        let mut instance = &mut config.builds[build_index];
        instance.count += 1;
        instance.time_total += result;
        instance.time_last = result;

        let complete = config
            .builds
            .iter()
            .all(|instance| instance.count == instance.iterations);

        if complete {
            config.completed = true;
            self.view.rewind();

            if self.new_line_first {
                term::newline().view(&mut self.view);
                self.new_line_first = false;
            }

            config.config.view(&mut self.view);
            " ".view(&mut self.view);
            config.avgs(&mut self.view);
            term::newline().view(&mut self.view);

            self.view.reset();
        }
        self.refresh();
    }

    pub fn refresh(&mut self) {
        self.view.rewind();

        term::newline().view(&mut self.view);

        "Running benchmarks:".view(&mut self.view);

        term::newline().view(&mut self.view);

        for config in self
            .configs
            .iter()
            .filter(|config| config.started && !config.completed)
        {
            let warmup = config
                .builds
                .iter()
                .any(|instance| instance.warmup_count < instance.warmups);

            if warmup {
                let count: usize = config
                    .builds
                    .iter()
                    .map(|instance| instance.warmup_count)
                    .sum();
                let total: usize = config.builds.iter().map(|instance| instance.warmups).sum();

                " - ".view(&mut self.view);
                config.config.view(&mut self.view);
                format!(" (warming up {}/{}) ", count, total).view(&mut self.view);
            } else {
                let count: usize = config.builds.iter().map(|instance| instance.count).sum();
                let total: usize = config
                    .builds
                    .iter()
                    .map(|instance| instance.iterations)
                    .sum();

                " - ".view(&mut self.view);
                config.config.view(&mut self.view);
                format!(" ({}/{}) ", count, total).view(&mut self.view);
            }

            config.avgs(&mut self.view);
            term::newline().view(&mut self.view);
        }

        term::newline().view(&mut self.view);

        let builds = self.configs[0].builds.len();
        let totals: Option<Vec<f64>> = (0..builds)
            .map(|build| {
                if self.configs.iter().any(|config| config.min_count() > 0) {
                    Some(
                        self.configs
                            .iter()
                            .map(|config| {
                                config.builds[build].avg(config.min_count()).unwrap_or(0.0)
                            })
                            .sum::<f64>(),
                    )
                } else {
                    None
                }
            })
            .collect();

        " - Current total ".view(&mut self.view);
        if let Some(totals) = totals {
            print_values(&totals, &mut self.view);
        }
        term::newline().view(&mut self.view);

        if builds > 1 {
            let summary: Option<Vec<f64>> = (0..builds)
                .map(|build| {
                    let instance_rel_sums: Vec<_> = self
                        .configs
                        .iter()
                        .filter_map(|config| {
                            match (
                                config.builds[build].avg(config.builds[build].count),
                                config.builds[0].avg(config.builds[0].count),
                            ) {
                                (Some(build), Some(first)) => Some(build / first),
                                _ => None,
                            }
                        })
                        .collect();
                    if instance_rel_sums.len() > 0 {
                        Some(
                            instance_rel_sums.iter().sum::<f64>()
                                / (instance_rel_sums.len() as f64),
                        )
                    } else {
                        None
                    }
                })
                .collect();

            " - Current summary ".view(&mut self.view);
            if let Some(summary) = summary {
                print_values(&summary, &mut self.view);
            }
            term::newline().view(&mut self.view);
        }

        self.view.flush();
    }

    pub fn complete(&mut self) {
        self.view.rewind();
        let iterations = self.configs[0].builds[0].iterations;

        let builds = self.configs[0].builds.len();
        let totals: Vec<f64> = (0..builds)
            .map(|build| {
                self.configs
                    .iter()
                    .map(|config| config.builds[build].avg(iterations).unwrap())
                    .sum::<f64>()
            })
            .collect();

        term::newline().view(&mut self.view);

        "Total ".view(&mut self.view);
        print_values(&totals, &mut self.view);
        term::newline().view(&mut self.view);

        if builds > 1 {
            let summary: Vec<f64> = (0..builds)
                .map(|build| {
                    let instance_rel_sum = self
                        .configs
                        .iter()
                        .map(|config| config.builds[build].time_total / config.builds[0].time_total)
                        .sum::<f64>();
                    instance_rel_sum / (builds as f64)
                })
                .collect();

            "Summary ".view(&mut self.view);
            print_values(&summary, &mut self.view);
            term::newline().view(&mut self.view);
        }

        term::newline().view(&mut self.view);

        self.view.flush();
    }
}
