import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// Registra en [LicenseRegistry] las licencias (OFL 1.1) de las dos fuentes
/// empaquetadas, para que aparezcan en la página de licencias de la app.
void registerFontLicenses() {
  const licenses = {
    'Plus Jakarta Sans': 'assets/fonts/OFL-PlusJakartaSans.txt',
    'JetBrains Mono': 'assets/fonts/OFL-JetBrainsMono.txt',
  };
  LicenseRegistry.addLicense(() async* {
    for (final MapEntry(key: package, value: asset) in licenses.entries) {
      final text = await rootBundle.loadString(asset);
      yield LicenseEntryWithLineBreaks([package], text);
    }
  });
}
