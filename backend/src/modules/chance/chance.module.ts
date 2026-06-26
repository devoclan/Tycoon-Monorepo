import { Module } from '@nestjs/common';
import { TypeOrmModule } from '@nestjs/typeorm';
import { Chance } from './entities/chance.entity';
import { ChanceService } from './chance.service';
import { ChanceController } from './chance.controller';
import { ChanceObservabilityService } from './chance-observability.service';
import { LoggerModule } from '../../common/logger/logger.module';

@Module({
  imports: [TypeOrmModule.forFeature([Chance]), LoggerModule],
  providers: [ChanceService, ChanceObservabilityService],
  controllers: [ChanceController],
  exports: [TypeOrmModule, ChanceObservabilityService],
})
export class ChanceModule {}
